// Traditional implementation of "EnterpriseBillingHub".
//
// This is a faithful, conventional TypeScript/Node implementation covering the
// SAME surface as the 150-line Nexus DSL program: schemas, governance policies,
// dynamic payment-provider binding, resource budgets, runtime validation
// (the manual equivalent of Nexus's formal SMT proof), business logic, events,
// LLM agents, a retrying workflow orchestrator, and scheduler wiring.
//
// It is included verbatim so the benchmark can count the tokens a human or an
// LLM must produce to build the same system the traditional way.

import { randomUUID } from "crypto";
import Stripe from "stripe";
import { Pool } from "pg";
import Redis from "ioredis";
import { z } from "zod";
import { EventEmitter } from "events";
import Anthropic from "@anthropic-ai/sdk";
import pino from "pino";

const logger = pino({ level: process.env.LOG_LEVEL ?? "info" });

// ============================================================
// 1. SCHEMAS (the equivalent of `thing` declarations)
// ============================================================

export const CustomerSchema = z.object({
  id: z.string().uuid(),
  email: z.string().email(),
  taxIdentifier: z.string(),
  accountStatus: z.enum(["active", "suspended", "closed"]),
});
export type Customer = z.infer<typeof CustomerSchema>;

export const InvoiceSchema = z.object({
  id: z.string().uuid(),
  customerId: z.string().uuid(),
  amount: z.number().nonnegative(),
  dueDate: z.string().datetime(),
  status: z.enum(["open", "settled", "failed"]),
});
export type Invoice = z.infer<typeof InvoiceSchema>;

export const ExecutionLogSchema = z.object({
  id: z.string().uuid(),
  eventOrigin: z.string(),
  elapsedTimeMs: z.number(),
  operationalCost: z.number(),
});
export type ExecutionLog = z.infer<typeof ExecutionLogSchema>;

// ============================================================
// 2. GOVERNANCE POLICIES (the equivalent of `policy`)
// ============================================================

export type Role = "AccountingSystem" | "SupportSystem" | "Admin";
export type TrustLevel = "unverified" | "reviewed" | "verified" | "proven";
const TRUST_ORDER: Record<TrustLevel, number> = {
  unverified: 0,
  reviewed: 1,
  verified: 2,
  proven: 3,
};

export interface SecurityContext {
  role: Role;
  trust: TrustLevel;
}

export class FinancialCompliancePolicy {
  // restrict write -> role.AccountingSystem
  static authorizeWrite(ctx: SecurityContext): void {
    if (ctx.role !== "AccountingSystem") {
      throw new Error("FinancialCompliance: write denied for role " + ctx.role);
    }
  }
  // encrypt Customer.tax_identifier
  static encryptTaxId(plaintext: string, key: Buffer): string {
    const crypto = require("crypto");
    const iv = crypto.randomBytes(12);
    const cipher = crypto.createCipheriv("aes-256-gcm", key, iv);
    const enc = Buffer.concat([cipher.update(plaintext, "utf8"), cipher.final()]);
    const tag = cipher.getAuthTag();
    return Buffer.concat([iv, tag, enc]).toString("base64");
  }
  // audit true
  static audit(action: string, ctx: SecurityContext): void {
    logger.info({ action, role: ctx.role, audit: true }, "audit-trail");
  }
}

export class ServiceLevelDefaults {
  static readonly defaultDeny = true;
  static enforceTrust(ctx: SecurityContext, min: TrustLevel): void {
    if (TRUST_ORDER[ctx.trust] < TRUST_ORDER[min]) {
      throw new Error(`trust ${ctx.trust} below required ${min}`);
    }
  }
}

// ============================================================
// 3. CAPABILITY: dynamic card-processor binding (the `capability`)
// ============================================================

export interface CardAuthorization {
  authorizationCode: string;
  carrier: string;
}

export interface CardProcessor {
  readonly name: string;
  readonly compliance: string[];
  readonly failureRate: number;
  charge(invoiceId: string, amount: number): Promise<CardAuthorization>;
}

class StripeProcessor implements CardProcessor {
  readonly name = "stripe";
  readonly compliance = ["PCI-DSS", "SCA"];
  readonly failureRate = 0.0007;
  private client: Stripe;
  constructor(apiKey: string) {
    this.client = new Stripe(apiKey, { apiVersion: "2024-06-20" });
  }
  async charge(invoiceId: string, amount: number): Promise<CardAuthorization> {
    const intent = await this.client.paymentIntents.create({
      amount: Math.round(amount * 100),
      currency: "usd",
      metadata: { invoiceId },
      confirm: true,
    });
    if (intent.status !== "succeeded") {
      throw new Error(`payment not completed: ${intent.status}`);
    }
    return { authorizationCode: intent.id, carrier: "stripe" };
  }
}

export class CardProcessorRegistry {
  private providers: CardProcessor[] = [];
  register(p: CardProcessor) {
    this.providers.push(p);
  }
  // bind the provider that meets compliance + failure-rate guarantees
  resolve(requiredCompliance: string[], maxFailureRate: number): CardProcessor {
    const eligible = this.providers
      .filter((p) => requiredCompliance.every((c) => p.compliance.includes(c)))
      .filter((p) => p.failureRate <= maxFailureRate)
      .sort((a, b) => a.failureRate - b.failureRate);
    if (eligible.length === 0) throw new Error("no eligible card processor");
    return eligible[0];
  }
}

// ============================================================
// 4. CONTRACTS: resource boundaries (the `contract`)
// ============================================================

export interface ResourceContract {
  maxMemoryBytes: number;
  maxCpuCores: number;
  executionTimeoutMs: number;
  financialBudget: number;
}

export const HighPrioritySLA: ResourceContract = {
  maxMemoryBytes: 2 * 1024 * 1024 * 1024,
  maxCpuCores: 1.5,
  executionTimeoutMs: 1500,
  financialBudget: 0.025,
};

async function withContract<T>(c: ResourceContract, fn: () => Promise<T>): Promise<T> {
  const start = Date.now();
  const timeout = new Promise<never>((_, rej) =>
    setTimeout(() => rej(new Error("execution_timeout exceeded")), c.executionTimeoutMs)
  );
  const result = await Promise.race([fn(), timeout]);
  const elapsed = Date.now() - start;
  if (elapsed > c.executionTimeoutMs) throw new Error("execution_timeout exceeded");
  return result as T;
}

// ============================================================
// 5. CONSTRAINTS: invariants (the `constraint`) — manual validation,
//    the hand-written equivalent of Nexus's formal SMT proof.
// ============================================================

export interface SystemState {
  systemLoad: number;
  callerTrust: TrustLevel;
  monthlyBudgetRemaining: number;
}

export function checkSecurityGuard(s: SystemState): void {
  if (!(s.systemLoad < 0.9)) throw new Error("SecurityGuard: system_load >= 0.90");
  if (TRUST_ORDER[s.callerTrust] < TRUST_ORDER["reviewed"])
    throw new Error("SecurityGuard: caller_trust_level < reviewed");
}

export function checkCostGuard(s: SystemState): void {
  if (!(s.monthlyBudgetRemaining > 5.0))
    throw new Error("CostGuard: monthly_budget_remaining <= $5.00");
}

// The no-negative-balance invariant must be hand-proven via tests here, because
// a traditional stack has no built-in symbolic verifier.
export function assertNoOverdraft(start: number, amount: number, minReserve: number): number {
  const balance = start - amount;
  if (balance < minReserve) {
    throw new Error(`invariant violated: balance ${balance} < reserve ${minReserve}`);
  }
  return balance;
}

// ============================================================
// 6. INTENTS: business logic (the `intent`)
// ============================================================

export async function processCardPayment(
  inv: Invoice,
  ctx: SecurityContext,
  state: SystemState,
  registry: CardProcessorRegistry,
  db: Pool
): Promise<{ authorization: CardAuthorization; receipt: string }> {
  checkCostGuard(state);
  FinancialCompliancePolicy.authorizeWrite(ctx);
  const processor = registry.resolve(["PCI-DSS", "SCA"], 0.001);
  const authorization = await withContract(HighPrioritySLA, () =>
    processor.charge(inv.id, inv.amount)
  );
  await db.query("UPDATE invoices SET status = $1 WHERE id = $2", ["settled", inv.id]);
  const receipt = generateTaxReceipt(inv, authorization);
  return { authorization, receipt };
}

function generateTaxReceipt(inv: Invoice, auth: CardAuthorization): string {
  return `RECEIPT ${inv.id} amount=${inv.amount} auth=${auth.authorizationCode}`;
}

// ============================================================
// 7. EVENTS (the `event` stream)
// ============================================================

export type BillingEvent =
  | { type: "InvoiceSettled"; invoiceId: string }
  | { type: "TransactionFailed"; invoiceId: string; reason: string }
  | { type: "NotificationDispatched"; invoiceId: string };

export const bus = new EventEmitter();
export function emit(e: BillingEvent) {
  bus.emit(e.type, e);
  logger.info({ event: e }, "billing-event");
}

// ============================================================
// 8. AGENTS: LLM collaborators (the `agent`)
// ============================================================

const anthropic = new Anthropic({ apiKey: process.env.ANTHROPIC_API_KEY });

export class SupportCommunicator {
  // metric ToneQuality: maximize satisfaction, minimize tokens
  async generateConfirmationEmail(inv: Invoice, customer: Customer): Promise<string> {
    const resp = await anthropic.messages.create({
      model: "claude-haiku-4-5-20251001",
      max_tokens: 300,
      system:
        "You write concise, warm payment-confirmation emails. Minimize tokens while maximizing customer satisfaction.",
      messages: [
        {
          role: "user",
          content: `Write a confirmation email for invoice ${inv.id}, amount $${inv.amount}, to ${customer.email}.`,
        },
      ],
    });
    const block = resp.content[0];
    return block.type === "text" ? block.text : "";
  }
}

export class InvoiceAuditor {
  // metric Accuracy: maximize consistency with past audits
  async validate(inv: Invoice, history: Invoice[]): Promise<boolean> {
    const resp = await anthropic.messages.create({
      model: "claude-haiku-4-5-20251001",
      max_tokens: 200,
      system: "You audit invoice calculations and detect anomalous discounts.",
      messages: [
        {
          role: "user",
          content: `Validate invoice ${JSON.stringify(inv)} against history ${JSON.stringify(
            history.slice(-5)
          )}. Reply VALID or ANOMALY.`,
        },
      ],
    });
    const block = resp.content[0];
    return block.type === "text" && block.text.includes("VALID");
  }
}

// ============================================================
// 9. WORKFLOW ORCHESTRATION with retry/backoff (the `workflow`)
// ============================================================

interface RetryOptions {
  retries: number;
  backoff?: "exponential" | "fixed";
  onFailure: () => Promise<void>;
}

async function runStep<T>(name: string, fn: () => Promise<T>, opts: RetryOptions): Promise<T> {
  let lastErr: unknown;
  for (let attempt = 1; attempt <= opts.retries + 1; attempt++) {
    try {
      return await fn();
    } catch (e) {
      lastErr = e;
      logger.warn({ step: name, attempt, err: String(e) }, "step-retry");
      if (attempt <= opts.retries) {
        const base = 100;
        const delay = opts.backoff === "exponential" ? base * 2 ** (attempt - 1) : base;
        await new Promise((r) => setTimeout(r, delay));
      }
    }
  }
  await opts.onFailure();
  throw lastErr;
}

export async function standardInvoiceSettlement(
  inv: Invoice,
  customer: Customer,
  ctx: SecurityContext,
  state: SystemState,
  deps: { registry: CardProcessorRegistry; db: Pool; redis: Redis }
): Promise<void> {
  checkSecurityGuard(state);

  // step ValidateInvoice (deterministic) retry 2 -> CancelWorkflow
  await runStep(
    "ValidateInvoice",
    async () => {
      InvoiceSchema.parse(inv);
      assertNoOverdraft(state.monthlyBudgetRemaining, inv.amount, 0);
    },
    { retries: 2, onFailure: async () => logger.error("workflow cancelled") }
  );

  // step ChargeAccount need:CardProcessor contract:HighPrioritySLA retry 3 backoff exp -> DispatchAlert
  const { authorization, receipt } = await runStep(
    "ChargeAccount",
    () => processCardPayment(inv, ctx, state, deps.registry, deps.db),
    {
      retries: 3,
      backoff: "exponential",
      onFailure: async () =>
        emit({ type: "TransactionFailed", invoiceId: inv.id, reason: "charge failed" }),
    }
  );
  emit({ type: "InvoiceSettled", invoiceId: inv.id });
  await deps.redis.set(`receipt:${inv.id}`, receipt);

  // step SendReceipt agent:SupportCommunicator reasoning:cost_optimized -> NotifyHumanSupervisor
  await runStep(
    "SendReceipt",
    async () => {
      const email = await new SupportCommunicator().generateConfirmationEmail(inv, customer);
      await deps.redis.set(`email:${inv.id}`, email);
      emit({ type: "NotificationDispatched", invoiceId: inv.id });
    },
    { retries: 0, onFailure: async () => logger.error("notify human supervisor") }
  );

  void authorization;
}

// ============================================================
// 10. SCHEDULER & OBSERVABILITY WIRING (the schedule/observe/optimize/secure)
// ============================================================

export function bootstrap(): {
  registry: CardProcessorRegistry;
  db: Pool;
  redis: Redis;
} {
  const registry = new CardProcessorRegistry();
  registry.register(new StripeProcessor(process.env.STRIPE_KEY ?? ""));
  const db = new Pool({ connectionString: process.env.DATABASE_URL });
  const redis = new Redis(process.env.REDIS_URL ?? "redis://localhost:6379");

  bus.on("InvoiceCreated", async (inv: Invoice) => {
    const customer = await fetchCustomer(inv.customerId, db);
    const ctx: SecurityContext = { role: "AccountingSystem", trust: "verified" };
    const state: SystemState = {
      systemLoad: await currentLoad(),
      callerTrust: "verified",
      monthlyBudgetRemaining: await budgetRemaining(redis),
    };
    await standardInvoiceSettlement(inv, customer, ctx, state, { registry, db, redis });
  });

  return { registry, db, redis };
}

async function fetchCustomer(id: string, db: Pool): Promise<Customer> {
  const r = await db.query("SELECT * FROM customers WHERE id = $1", [id]);
  return CustomerSchema.parse(r.rows[0]);
}
async function currentLoad(): Promise<number> {
  const os = require("os");
  return os.loadavg()[0] / os.cpus().length;
}
async function budgetRemaining(redis: Redis): Promise<number> {
  const v = await redis.get("budget:remaining");
  return v ? parseFloat(v) : 0;
}
