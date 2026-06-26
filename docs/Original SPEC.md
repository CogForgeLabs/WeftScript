# **Architecture and Specifications of the Nexus Universal Intent Operating System and Software Fabric**

## **Paradigm Shift: From Code Compilation to Intent-Based Software Fabric**

The field of software engineering has reached a critical inflection point. Traditional programming paradigms, defined by the translation of human thoughts into deterministic, line-by-line machine instructions, have introduced significant cognitive and systemic overhead1. Developers must simultaneously manage syntactic correctness, memory hierarchies, network topologies, dependency graphs, security policies, and deployment orchestrations3. When artificial intelligence agents are introduced into this legacy workflow, they inherit these exact complexities, often generating brittle code, experiencing context window exhaustion, or failing to maintain systemic coherence across large, distributed directories1.  
The Nexus Universal Intent Operating System addresses these challenges by replacing the traditional execution stack with an intent-based software fabric1. In this paradigm, software is no longer authored as a sequence of text-based instructions7. Instead, it is declared as a persistent, self-describing, and content-addressed graph of mathematical intentions, resource contracts, security policies, and data dependencies5. The text-based Domain-Specific Language (DSL) is not the code itself; it is merely one of several bi-directionally synchronized graphical projections of this underlying execution graph8.

Legacy Pipeline:  Imperative Source Code ──► Text Parser ──► Compiler AST ──► Machine Binary  
Nexus Pipeline:   Intent Declaration   ──► Unified Graph ──► Multi-Objective Planner ──► Target Runtimes

At the core of this architecture is the transformation of computation into a continuous knowledge loop1. Standard software execution behaves as a transient process where inputs yield outputs and the system state is subsequently lost. The Nexus paradigm establishes a closed-loop system where every execution cycle feeds operational metrics, financial costs, model performance, latency distributions, and human evaluations directly back into a persistent execution knowledge graph11. The planning engine analyzes this data to dynamically optimize the system's execution pathways12. This design allows future workflows to automatically benefit from historical operational profiles, matching or exceeding the performance of hand-tuned systems over time13.

| Architectural Dimension | Traditional Software Systems | Nexus Universal Intent Operating System |
| :---- | :---- | :---- |
| **Primary Source of Truth** | Mutable textual source files stored in filesystems7 | Immutable, content-addressed graph database7 |
| **Operational Interface** | Strict syntax-first imperative code blocks1 | Declarative specifications of goals and constraints1 |
| **Dependency Resolution** | Semantic versioning and explicit package registries7 | Semantic capability discovery via runtime registries18 |
| **Execution Targeting** | Hardcoded compilation targets and runtimes20 | Dynamic target mapping via multi-objective planners12 |
| **Optimization Method** | Manual code refactoring and profiling | Continuous Bayesian feedback and runtime planning11 |
| **Concurrency Paradigm** | Explicit async/await, threading, or lock primitives | Implicit, auto-parallelism driven by graph dependencies21 |
| **Telemetry & Observability** | External agent instrumentation and sidecar loggers | Native structural telemetries embedded in graph nodes |
| **Security and Sandboxing** | Ambient system authority with runtime sandbox layers | Static capability contracts and default-deny structures4 |

## **The Core Graph Architecture: Primitives, Separation, and Semantic Git**

To achieve complete system interoperability while maintaining a small, verifiable core, Nexus organizes all software declarations around seven fundamental, typed primitives. Any complex computational construct—from simple functions and relational databases to multi-agent intelligence networks—is represented as a composition of these seven mathematical objects.

### **The Seven True Primitives**

       ┌───────────┐       ┌────────────┐       ┌────────────┐  
       │   Thing   ├──────►│   Intent   │◄──────┤ Constraint │  
       └─────┬─────┘       └─────┬──────┘       └─────┬──────┘  
             │                   │                    │  
             ▼                   ▼                    ▼  
       ┌───────────┐       ┌────────────┐       ┌────────────┐  
       │  Policy   │       │   Event    │       │ Capability │  
       └───────────┘       └────────────┘       └────────────┘  
                                 │  
                                 ▼  
                           ┌────────────┐  
                           │  Contract  │  
                           └────────────┘

The unified graph model eliminates arbitrary programming language abstractions, ensuring that all aspects of an application compile into the exact same directed acyclic graph (DAG) structure20.

| Primitive | Mathematical Symbol | Core Functional Role | Concrete Definition Example |
| :---- | :---- | :---- | :---- |
| **Thing** | **![][image1]** | Defines structural schemas, state properties, and interfaces23 | thing Account { balance: Decimal, currency: String } |
| **Intent** | **![][image2]** | Expresses functional goals or state transitions to execute1 | intent SettleInvoice { invoice\_id: UUID } |
| **Constraint** | **![][image3]** | Enforces boundary conditions on executions or states4 | constraint execution\_time \< 50ms |
| **Policy** | **![][image4]** | Controls security, access rights, and governance boundaries4 | policy SecureData { encrypt: Account.balance } |
| **Event** | **![][image5]** | Signals asynchronous state changes across system nodes9 | event CustomerSubscribed { account\_id: UUID } |
| **Capability** | **![][image6]** | Declares an abstract functional requirement for matching18 | need: identity\_verifier |
| **Contract** | **![][image7]** | Defines performance, cost, and safety agreements19 | contract FinancialSLA { max\_cost: $0.05, reliability: 0.999 } |

### **Triple-Graph Partitioning: Facts, Plans, and Execution**

To prevent the dynamic behavior of complex distributed runtimes from introducing instability into static system specifications, the Nexus architecture splits its global state into three decoupled, interacting graphs:

┌─────────────────────────────────┐  
│     Knowledge Graph (Facts)     │ \<── Immutable schema definitions & state properties  
└──────────────┬──────────────────┘  
               │ Triggers  
               ▼  
┌─────────────────────────────────┐  
│       Plan Graph (Plans)        │ \<── Abstract solver paths, constraints, & requirements  
└──────────────┬──────────────────┘  
               │ Binds  
               ▼  
┌─────────────────────────────────┐  
│    Execution Graph (Runtime)    │ \<── Active processes, system links, & data flows  
└─────────────────────────────────┘

The **Knowledge Graph** acts as the static foundation of the application. It maps the schema properties of every Thing and the rules defined by every Policy4. Because this graph is content-addressed, its topology is completely immutable7. It forms a stable, queryable index of application assets, accessible to human designers, verification engines, and AI agents alike7.  
The **Plan Graph** is a dynamic projection created by the planner20. When an execution trigger occurs, the planner compiles the relevant static intents, constraints, and capabilities into an abstract DAG of logical operations16. This graph models the logical dependencies and processing stages required to satisfy the system's operational goals, but remains independent of physical resource or hardware allocations16.  
The **Execution Graph** represents the active, physical state of the application9. It maps logical plan nodes to concrete runtime processes, database engines, external API connections, or microservices19. The execution graph changes dynamically to route around node failures, shift compute loads across networks, scale out resource nodes, and manage data caching7.

### **Semantic Git and Content-Addressed Syntax Graphs**

By representing the codebase as a typed, content-addressed graph database rather than a collection of flat, text-based files, the platform eliminates traditional compilation, linking, and versioning bottlenecks7. Every node in the application is identified by a cryptographically secure 512-bit BLAKE3 content identifier (CID)7:  
![][image8]  
This content-addressed paradigm provides native code provenance and security7. Because the identifiers are computed directly from the immutable syntax tree, a developer can change structural names locally without breaking downstream dependencies7. The execution engine continues to reference the immutable, content-addressed mathematical nodes7.  
"Semantic Git" operates directly on this content-addressed graph database, transforming version control8. Traditional version control systems store textual line-by-line insertions and deletions, which frequently leads to merge conflicts, broken builds, and lost syntactic context7.  
Semantic Git registers modifications as structured transformations of graph vertices and edges10. It records changes as explicit semantic patches (e.g., *Add field customer\_id to Thing Order*), maintaining complete mathematical validity at every point in the development history15.

## **The Unified Compiler Pipeline and Symbolic Verification**

The compilation of a Nexus application is designed as a series of structured, sequential passes rather than a direct code-generation process. This design treats artificial intelligence as an integrated optimization stage, rather than forcing it to write complex, error-prone syntax from scratch28.

                     ┌───────────────────────┐  
                     │   Sourcing & Parsing  │  
                     └──────────┬────────────┘  
                                │  
                                ▼  
                     ┌───────────────────────┐  
                     │   Planning Stage      │  
                     └──────────┬────────────┘  
                                │  
                                ▼  
                     ┌───────────────────────┐  
                     │   Optimization Stage  │  
                     └──────────┬────────────┘  
                                │  
                                ▼  
                     ┌───────────────────────┐  
                     │   AI Synthesis Pass   │  
                     └──────────┬────────────┘  
                                │  
                                ▼  
                     ┌───────────────────────┐  
                     │  Formal SMT Proof     │  
                     └──────────┬────────────┘  
                                │  
                                ▼  
                     ┌───────────────────────┐  
                     │   Execution Binding   │  
                     └───────────────────────┘

### **Compiler Pipeline Execution Phases**

* **Sourcing and Parsing:** The compiler ingests visual graph schemas, textual DSL files, or direct API modifications, resolving them into a single, unified Abstract Syntax Graph (ASG)10.  
* **Planning Stage:** The planner evaluates declared intents against constraints and security policies to generate a logical Directed Acyclic Graph (DAG)16.  
* **Optimization Stage:** The compiler determines optimal hardware selections, data models, memory layouts, and caching configurations using multi-objective algorithms14.  
* **AI Synthesis Pass:** AI agents analyze declarative signatures to generate prompt contexts, refine semantic structures, and tune model parameters for probabilistic nodes28.  
* **Formal SMT Proof:** The compiler translates path conditions into SMT constraints, proving safety invariants and verifying resource contracts31.  
* **Execution Binding:** The compiler generates target bindings and loads them into the Rust core engine to initiate runtime execution across active distributed environments12.

### **Determinism, Provenance, and SMT-Based Path Verification**

To balance high-performance compute logic with flexible AI workflows, the Nexus compiler enforces a clear distinction between deterministic and probabilistic execution nodes3.  
A deterministic node (e.g., *Execute payment calculation in Rust*) guarantees verifiable, mathematically repeatable outputs3. A probabilistic node (e.g., *Generate summary of contract dispute using an LLM*) produces outputs that are inherently variable3.  
The compiler verifies these boundaries statically. Probabilistic nodes are isolated from safety-critical system paths and wrapped in automated evaluation suites4.  
This verification is powered by an SMT-based Symbolic Execution engine31. The compiler translates path constraints into assertions compatible with solvers such as Z3 or CVC431.  
For example, given a financial state transition, the compiler validates that no valid execution path can result in negative balances:

Code snippet  
guarantee  
    all Path p \=\> p.balance \>= p.minimum\_reserve

This statement is parsed, compiled into a logical path constraint, and verified using piecewise symbolic composition32. If the solver identifies a violating path, the build is rejected, and the compiler generates a target test case illustrating the vulnerability31.  
To ensure auditability across these mixed workloads, the platform implements a native provenance mechanism4. Every item produced by an execution pipeline is tagged with a cryptographic lineage chain, allowing developers and auditors to trace outputs back to their origins:  
![][image9]

### **Time-Travel and State Replay Semantics**

The platform's content-addressed database architecture enables native time-travel debugging and execution replay7. Because the application state is maintained as an append-only graph of immutable versions, the runtime can reconstruct the exact system state at any historical point in time27:

Code snippet  
query SystemState as\_of "2026-06-17T14:30:00Z"

This query instructs the engine to re-map the active pointers of the Knowledge and Execution graphs to the historical content hashes valid at that timestamp7. This allows developers to reproduce production bugs, audit financial ledger states, and validate historical model behaviors with complete bit-level accuracy7.

## **High-Performance Scheduling and Resource-Constrained Execution**

To support complex distributed networks, the platform replaces manually orchestrated asynchronous code (such as async/await, concurrent threads, or explicit worker queues) with dependency-driven execution scheduling21.

### **Native Auto-Parallelism and DAG Resolution**

                     Original Intent:  
                     \[Step A\] and \[Step B\] can run independently.  
                     \[Step C\] requires output of \[Step A\].  
                     \[Step D\] requires output of \[Step B\].

                     Dynamic Graph Translation:  
                           \[Step A\] ──► \[Step C\]  
                              ||           │  
                           \[Step B\] ──► \[Step D\]  
                              (Executed in Parallel)

The compiler evaluates the data dependency vectors of the Plan Graph, automatically isolating and running independent execution paths in parallel21. Because execution is structured as a DAG, the scheduler runs non-dependent tasks concurrently, managing lock allocations, resource distributions, and distributed cluster routing automatically22.

### **Resource-Aware Planning and Multi-Objective Edge Execution**

Let ![][image10] represent a task DAG, where each computational vertex ![][image11] requires a runtime memory footprint ![][image12] and a processor capability execution value ![][image13] on a target machine ![][image14]21.  
The system scheduler operates as a multi-objective optimization solver, balancing execution latency, infrastructure costs, and energy usage12. The scheduling target is modeled as a multi-objective Pareto optimization problem14:  
![][image15]  
This optimization is subject to the strict constraint that the active peak memory footprint must never exceed physical processor capacities21:  
![][image16]  
where ![][image17] is the start time, ![][image18] is the finish time, and ![][image19] is the physical memory capacity of node ![][image14]21.  
To solve this scheduling problem, the system uses a hybrid approach:

* **Heterogeneous Earliest Finish Time (HEFT):** Static task priorities are calculated using task bottom-levels, mapping tasks to processors that minimize execution and communication latency21.  
* **Multi-Objective Reinforcement Learning (MORL):** Real-time task allocation is optimized using Proximal Policy Optimization (PPO)14. This allows the system to adapt dynamically to changing resource availability on edge servers or local devices12.

The scheduler also evaluates whether a task *should* run based on its current operational budget12. If a task is projected to exceed its financial or time limits, the planner alters the execution plan, routing tasks through lower-cost resources or fallback pathways12.

### **Local-First Resilience**

The architecture is built on local-first principles. The compiler, scheduler, database, and runtimes are designed to run locally on a single machine.  
If internet or network connectivity is severed, the system continues to process executions using local components7.  
When network connections are restored, the platform synchronizes the local changes with the wider distributed network, resolving concurrent updates using Triple Graph Grammars (TGGs) and semantic state reconciliation36.

## **Dynamic Capability Discovery and Universal Abstractions**

To achieve true language and runtime agnosticism, the platform decouples components using semantic capability contracts, completely avoiding static library imports6.

### **Capability Resolution and Model Context Protocol (MCP)**

                     Declared Need: need: ocr\_scanner  
                                       │  
                                       ▼  
                       Model Context Protocol Engine  
                   ┌───────────────────┴───────────────────┐  
                   ▼                   ▼                   ▼  
            \[Tesseract OCR\]     \[Google Vision\]     \[AWS Textract\]  
            Type: Local WASM    Type: Remote API    Type: Remote API  
            Trust: Proven       Trust: Verified     Trust: Reviewed  
            Cost: $0.00         Cost: $0.015/Call   Cost: $0.025/Call

Instead of manually importing libraries (e.g., import stripe or const stripe \= require('stripe')), developers declare an abstract functional need7. The runtime evaluates these requirements using the Model Context Protocol (MCP)19.  
MCP uses JSON-RPC 2.0 communication to discover, query, and bind external interfaces dynamically at runtime19. This allows the orchestrator to evaluate available capabilities on the fly and bind the most efficient provider based on the active execution context18.

### **Universal Data and Model Routing**

Data persistence is abstracted through a unified storage interface. Developers declare structural entities and their relationships without defining target database engines:

Code snippet  
store Customer

The runtime analyzes the schema structure and historical access patterns to select the most efficient database storage option:

* **Relational Data Storage:** Routed to Postgres, DuckDB, or SQLite for complex structured queries7.  
* **High-Volume Key-Value Caching:** Structured using Redis or local RocksDB instances.  
* **Semantic Document Search:** Indexed in Vector databases for retrieval-augmented generation tasks19.

A similar abstraction layer manages generative model execution. Rather than hardcoding connections to specific AI endpoints (e.g., calling gpt-4o directly), developers specify a logical reasoning requirement:

Code snippet  
reasoning level.deep\_reasoning

The runtime acts as an AI Router, dynamically selecting the optimal model based on operational cost bounds, output token budgets, and availability constraints13.

### **DSPy-Style Prompt Evolution and Bayesian Optimization**

For pipelines that interact with generative AI models, the platform replaces manual prompt tuning with compiler-driven prompt optimization28.

┌─────────────────────────────────┐  
│     Signature Specification     │ \<── Declares input/output interfaces & metrics  
└──────────────┬──────────────────┘  
               │ Compile  
               ▼  
┌─────────────────────────────────┐  
│     Bayesian Optimizer (TuRBO)  │ \<── Explores instruction prompts & few-shot demos \[cite: 29, 40, 41\]  
└──────────────┬──────────────────┘  
               │ Evaluate  
               ▼  
┌─────────────────────────────────┐  
│     Optimized Model Program     │ \<── Deploys high-performance, cost-efficient prompts  
└─────────────────────────────────┘

The developer defines a structured signature containing the inputs, outputs, and validation metrics of a task28. The compiler uses DSPy-style optimization loops to automatically discover, evaluate, and deploy high-performance prompt instructions and few-shot examples13.  
This optimization process uses Bayesian Optimization (such as Trust Region Bayesian Optimization, or TuRBO) to systematically balance prompt exploration and exploitation11.  
The optimizer evaluates candidate prompts against the specified metrics, iteratively refining instructions to maximize accuracy while minimizing API costs11.

## **Triple-View IDE, Prompt Optimizer Legacy, and Simulation Engine**

### **Triple-View Synchronisation and Projectional Editing**

The platform is managed through a visual Triple-View Integrated Development Environment (IDE), providing three bi-directionally synchronized interfaces:

* **Graph View:** Represents the system visually as interactive nodes and edges, allowing developers to manage workflows, schemas, and constraint configurations graphically10.  
* **DSL View:** Projects the underlying graph as clear, readable declarative text, enabling power users to author and manage system specifications efficiently8.  
* **Generated Runtime View:** Displays live execution parameters, cost distributions, telemetry streams, and dynamic model routing tables8.

Consistency across these three views is maintained using Triple Graph Grammars (TGGs)36. A TGG formally defines the structural correspondence between the graph representation, the textual DSL syntax, and the runtime intermediate representation36.  
When an edit is made in any single view, the TGG engine propagates the update incrementally across the other two projections in real-time37.

                     ┌──────────────────┐  
                     │ Source Graph (S) │ \<── Graph View (Visual) \[cite: 36\]  
                     └────────┬─────────┘  
                              │  
                     ┌────────┴─────────┐  
                     │  Corres. Link (C)│ \<── Triple Graph Grammars (TGG) \[cite: 36\]  
                     └────────┬─────────┘  
                              │  
                     ┌────────┴─────────┐  
                     │ Target Graph (T) │ \<── DSL View (Textual) \[cite: 36\]  
                     └──────────────────┘

This model-driven approach ensures that non-technical users can design systems visually, developers can edit them in text, and AI agents can manipulate the underlying graph directly10. It eliminates the need for complex, traditional text parsers, preventing compilation failures during collaborative design10.

### **Visual Prompt Optimizer Tiers and Shared Registries**

The IDE supports collaborative development through a built-in Marketplace and global prompt registry, monetization structures, and access tiers45.

| Platform Tier | Subscription Fee | Key Features and Constraints | Target Audience |
| :---- | :---- | :---- | :---- |
| **Community Free Tier** | Free | Limit of 10 active workflows; 5 compiler downloads per month; 10 hosted test runs using custom API keys45. | Hobbyists, solo developers, and educational users. |
| **Professional Tier** | Flat, low-cost monthly subscription | Unlimited workflows and execution targets; native Bayesian optimizer access; automatic benchmarking; hosted evaluations via OpenRouter45. | Professional developers, startups, and agile engineering teams. |
| **Enterprise Governance Tier** | Custom pricing structures | Single Sign-On (SSO); data lineage tracking; custom SLA contracts; advanced SMT proof checks4. | Highly regulated enterprises, financial institutions, and large security-focused organizations. |

### **Digital Twin Simulation Mode**

Before code is deployed to active production systems, the platform can evaluate it inside a built-in Digital Twin Simulation Mode. This mode executes plans in a simulated sandbox environment using historical production telemetry21.  
It generates predictive impact analysis reports detailing execution latencies, API costs, model performance, and security compliance4. This allows developers to catch bugs, cost spikes, or security violations before a single production action is taken3.

## **Nexus DSL Formal Grammar and Reference Program**

### **Core Syntax Rules**

The textual representation of the Nexus DSL uses a clean, context-free grammar designed for easy reading and manipulation by both humans and AI models16.  
The structural grammar rules are defined below in Extended Backus-Naur Form (EBNF):

EBNF  
Program         ::= ProjectDecl Block\*  
ProjectDecl     ::= "project" Identifier  
Block           ::= ThingDecl | IntentDecl | ConstraintDecl | PolicyDecl | WorkflowDecl | AgentDecl  
ThingDecl       ::= "thing" Identifier \[ "implements" Identifier \] "{" Field\* "}"  
Field           ::= Identifier ":" Type \[ "policy" Identifier \]  
IntentDecl      ::= "intent" Identifier "{" "goal" Statement "}"  
ConstraintDecl  ::= "constraint" Identifier "{" Expression "}"  
PolicyDecl      ::= "policy" Identifier "{" SecurityDirective\* "}"  
WorkflowDecl    ::= "workflow" Identifier "{" Trigger Step\* "}"  
Step            ::= "step" Identifier "{" Action "}"  
AgentDecl       ::= "agent" Identifier "{" Goal Metric "}"  
Type            ::= "String" | "Decimal" | "UUID" | "Timestamp" | "List\[" Type "\]" | Identifier

### **Enterprise Reference Program**

The following specification implements a transactional billing and customer communication system, demonstrating the 7 core primitives, dynamic capabilities, scheduling, policies, and multi-agent flows working in unison.

Code snippet  
project EnterpriseBillingHub

\# \==========================================  
\# 1\. POLICIES (Governance & Cryptography)  
\# \==========================================

policy FinancialCompliance  
    restrict write \-\> role.AccountingSystem  
    encrypt Customer.tax\_identifier  
    audit true

policy ServiceLevelDefaults  
    trust verified  
    default\_deny true

\# \==========================================  
\# 2\. THINGS (Data Schemas & Interfaces)  
\# \==========================================

thing Customer  
    id: UUID  
    email: String  
    tax\_identifier: String policy FinancialCompliance  
    account\_status: String

thing Invoice  
    id: UUID  
    customer\_id: UUID  
    amount: Decimal  
    due\_date: Timestamp  
    status: String

thing ExecutionLog  
    id: UUID  
    event\_origin: String  
    elapsed\_time\_ms: Decimal  
    operational\_cost: Decimal

\# \==========================================  
\# 3\. CAPABILITIES (Dynamic Provider Binding)  
\# \==========================================

capability CardProcessor  
    signature  
        input: invoice\_id: UUID, amount: Decimal  
        output: authorization\_code: String, carrier: String  
    guarantee  
        compliance: \["PCI-DSS", "SCA"\]  
        max\_failure\_rate: 0.001

\# \==========================================  
\# 4\. CONTRACTS (Resource Boundaries)  
\# \==========================================

contract HighPrioritySLA  
    max\_memory: 2GB  
    max\_cpu\_cores: 1.5  
    execution\_timeout: 1500ms  
    financial\_budget: $0.025

\# \==========================================  
\# 5\. CONSTRAINTS (Mathematical Invariants)  
\# \==========================================

constraint SecurityGuard  
    system\_load \< 0.90  
    caller\_trust\_level \>= trust.reviewed

constraint CostGuard  
    monthly\_budget\_remaining \> $5.00

\# \==========================================  
\# 6\. INTENTS (Abstract Transformations)  
\# \==========================================

intent ProcessCardPayment  
    input: inv: Invoice  
    goal  
        settle\_invoice\_balance  
        generate\_tax\_receipt  
    constraint CostGuard

\# \==========================================  
\# 7\. EVENTS (Asynchronous Signal Stream)  
\# \==========================================

event InvoiceSettled  
event TransactionFailed  
event NotificationDispatched

\# \==========================================  
\# 8\. MULTI-AGENT COLLABORATION  
\# \==========================================

agent InvoiceAuditor  
    goal  
        validate\_invoice\_calculations  
        detect\_anomalous\_discounts  
    metric Accuracy  
        maximize consistency\_with\_past\_audits  
    tools: \[DatabaseViewer, DynamicCalculator\]

agent SupportCommunicator  
    goal  
        generate\_payment\_confirmation\_email  
    metric ToneQuality  
        maximize customer\_satisfaction  
        minimize output\_token\_count  
    constraint CostGuard

\# \==========================================  
\# 9\. RE-ENTRANT WORKFLOWS (DCR Graph)  
\# \==========================================

workflow StandardInvoiceSettlement  
    trigger on Event.InvoiceCreated  
      
    step ValidateInvoice  
        intent ValidateTaxStructure  
        deterministic true  
        fallback  
            retry 2  
            on\_failure CancelWorkflow  
              
    step ChargeAccount  
        intent ProcessCardPayment  
        need: CardProcessor  
        contract HighPrioritySLA  
        fallback  
            retry 3  
            backoff exponential  
            on\_failure DispatchAlert  
              
    step SendReceipt  
        intent SendConfirmationEmail  
        agent SupportCommunicator  
        reasoning level.cost\_optimized  
        tools: \[EmailGateway\]  
        fallback  
            on\_failure NotifyHumanSupervisor

\# \==========================================  
\# 10\. SCHEDULER & OPTIMIZATION POLICIES  
\# \==========================================

schedule StandardInvoiceSettlement realtime  
observe auto  
optimize cost  
secure auto

## **Architectural Conclusions and Implementation Roadmap**

The Nexus Universal Intent Operating System addresses the inherent complexities of modern distributed systems and AI workflows1. By representing software as a structured, content-addressed graph rather than flat text files, the platform simplifies visual coordination, enables formal SMT-based safety verification, and allows AI models to assist with system design safely within defined semantic boundaries4.

### **Phased Engineering Implementation Roadmap**

The platform's development is organized into three progressive, six-month milestones:

* **Phase 1: Compiler & AST Core (Months 1-6):** Building the Rust-based graph storage library, implementing BLAKE3 content-addressing, parsing the basic DSL grammar, and constructing the core local-first runtime interface7.  
* **Phase 2: Formal Verification & Orchestration (Months 7-12):** Integrating the Z3 SMT solver for path constraint checks, implementing the HEFT scheduling engine, and establishing multi-language runtime connectors for Python and Node.js21.  
* **Phase 3: Multi-Agent Synthesis & IDE (Months 13-18):** Launching the visual Triple-View IDE with TGG synchronization, deploying Model Context Protocol capability registries, and integrating the Bayesian Optimization engine for automated prompt tuning11.

By building a system around verified intentions rather than static instructions, the Nexus platform provides a robust foundation for building resilient, adaptable, and highly optimized distributed applications1.

#### **Works cited**

1. The Shift From Programming Languages to Intent Languages \- DEV Community, [https://dev.to/jaideepparashar/the-shift-from-programming-languages-to-intent-languages-365a](https://dev.to/jaideepparashar/the-shift-from-programming-languages-to-intent-languages-365a)  
2. Intent-based Programming: | by Brandon L. Townes \- Medium, [https://medium.com/@brandonltownes/intent-based-programming-a0476d247d6a](https://medium.com/@brandonltownes/intent-based-programming-a0476d247d6a)  
3. Beyond the Vibe | Chris Villanueva, [https://chrisvillanueva.com/posts/beyond-the-vibe-engineering-reliable-distributed-dystems-in-the-ai-era/](https://chrisvillanueva.com/posts/beyond-the-vibe-engineering-reliable-distributed-dystems-in-the-ai-era/)  
4. Model to mitigate: Using DCR graphs to prevent vulnerabilities in smart contracts \- research.chalmers.se, [https://research.chalmers.se/publication/552497/file/552497\_Fulltext.pdf](https://research.chalmers.se/publication/552497/file/552497_Fulltext.pdf)  
5. From Agents to Intent \- Ben Houston, [https://ben3d.ca/blog/declarative-intent-manifesto](https://ben3d.ca/blog/declarative-intent-manifesto)  
6. The Architecture Tax Nobody Talks About: Why Your AI Agents Can't Find Each Other, [http://tkthetechie.io/blog/the-architecture-tax-nobody-talks-about-why-your-ai-agents-cant-find-each-other](http://tkthetechie.io/blog/the-architecture-tax-nobody-talks-about-why-your-ai-agents-cant-find-each-other)  
7. The big idea · Unison programming language, [https://www.unison-lang.org/docs/the-big-idea/](https://www.unison-lang.org/docs/the-big-idea/)  
8. Projectional Editing \- Martin Fowler, [https://martinfowler.com/bliki/ProjectionalEditing.html](https://martinfowler.com/bliki/ProjectionalEditing.html)  
9. Exformatics Declarative Case Management Workflows as DCR Graphs \- ResearchGate, [https://www.researchgate.net/publication/261843648\_Exformatics\_Declarative\_Case\_Management\_Workflows\_as\_DCR\_Graphs](https://www.researchgate.net/publication/261843648_Exformatics_Declarative_Case_Management_Workflows_as_DCR_Graphs)  
10. Projectional Editing: The Future of Programming \- DZone, [https://dzone.com/articles/projectional-editing-the-future-of-programming-1](https://dzone.com/articles/projectional-editing-the-future-of-programming-1)  
11. Bayesian optimization for chemical reactions \- RSC Publishing, [https://pubs.rsc.org/en/content/articlehtml/2026/cs/d5cs00962f](https://pubs.rsc.org/en/content/articlehtml/2026/cs/d5cs00962f)  
12. Resource-Aware Dynamic Scheduling for Tasks With Deadline Constraints on Edge Computing Systems, [https://www.computer.org/csdl/journal/cc/2025/04/11143954/29yj3hhJDri](https://www.computer.org/csdl/journal/cc/2025/04/11143954/29yj3hhJDri)  
13. GEPA optimization \- DSPy, [https://dspy.ai/getting-started/gepa-optimization/](https://dspy.ai/getting-started/gepa-optimization/)  
14. Resource Scheduling Algorithm for Edge Computing Networks Based on Multi-Objective Optimization \- MDPI, [https://www.mdpi.com/2076-3417/15/19/10837](https://www.mdpi.com/2076-3417/15/19/10837)  
15. Unison is a programming language that explores the ramifications of making code immutable and stored in a database, instead of a set of text files \- Reddit, [https://www.reddit.com/r/programming/comments/1doosdi/unison\_is\_a\_programming\_language\_that\_explores/](https://www.reddit.com/r/programming/comments/1doosdi/unison_is_a_programming_language_that_explores/)  
16. Declarative Workflows \- Overview \- Microsoft Learn, [https://learn.microsoft.com/en-us/agent-framework/workflows/declarative](https://learn.microsoft.com/en-us/agent-framework/workflows/declarative)  
17. A look at Unison: a revolutionary programming language | Hacker News, [https://news.ycombinator.com/item?id=34307552](https://news.ycombinator.com/item?id=34307552)  
18. draft-wang-dmsc-terminology-ioa-networking-00 \- Terminology for Networking Infrastructure in the Internet of Agents \- IETF Datatracker, [https://datatracker.ietf.org/doc/draft-wang-dmsc-terminology-ioa-networking/](https://datatracker.ietf.org/doc/draft-wang-dmsc-terminology-ioa-networking/)  
19. What is the Model Context Protocol (MCP)? \- Databricks, [https://www.databricks.com/blog/what-is-model-context-protocol](https://www.databricks.com/blog/what-is-model-context-protocol)  
20. US20130263099A1 \- Common intermediate representation for data scripting language \- Google Patents, [https://patents.google.com/patent/US20130263099A1/en](https://patents.google.com/patent/US20130263099A1/en)  
21. Memory-aware Adaptive Scheduling of Scientific Workflows on Heterogeneous Architectures \- arXiv, [https://arxiv.org/html/2503.22365v1](https://arxiv.org/html/2503.22365v1)  
22. DAG-based Task Planner Overview \- Emergent Mind, [https://www.emergentmind.com/topics/dag-based-task-planner](https://www.emergentmind.com/topics/dag-based-task-planner)  
23. Hashing and Graphs | CSE 373, [https://courses.cs.washington.edu/courses/cse373/24su/lessons/hashing-and-graphs/](https://courses.cs.washington.edu/courses/cse373/24su/lessons/hashing-and-graphs/)  
24. A Compositional Approach to Mapping BPMN to DCR for Cross-Paradigm Process Modeling \- DTU Research Database, [https://orbit.dtu.dk/files/440424875/Camera\_Ready\_Sound\_encodings\_from\_BPMN\_to\_DCR.pdf](https://orbit.dtu.dk/files/440424875/Camera_Ready_Sound_encodings_from_BPMN_to_DCR.pdf)  
25. Content-addressed data and CIDs \- S5 Network Docs, [https://docs.sfive.net/concepts/content-addressed-data.html](https://docs.sfive.net/concepts/content-addressed-data.html)  
26. Content-Based Addressing: Mechanisms & Applications \- Emergent Mind, [https://www.emergentmind.com/topics/content-based-addressing](https://www.emergentmind.com/topics/content-based-addressing)  
27. Here's what's been happening with Unison · Unison programming language, [https://www.unison-lang.org/blog/heres-whats-been-happening-with-unison/](https://www.unison-lang.org/blog/heres-whats-been-happening-with-unison/)  
28. DSPy Prompt Optimization: Hands-On Guide for Success \- ADaSci, [https://adasci.org/blog/dspy-streamlining-llm-prompt-optimization](https://adasci.org/blog/dspy-streamlining-llm-prompt-optimization)  
29. What is DSPy? \- IBM, [https://www.ibm.com/think/topics/dspy](https://www.ibm.com/think/topics/dspy)  
30. Freon Background — The Vision and Principles Behind the Language Workbench – What is Projectional Editing?, [https://www.freon4dsl.dev/Background/Projectional\_Editing](https://www.freon4dsl.dev/Background/Projectional_Editing)  
31. What Is Symbolic Execution? How It Works & Limitations \- Apiiro, [https://apiiro.com/glossary/symbolic-execution/](https://apiiro.com/glossary/symbolic-execution/)  
32. Symbolic Execution in Practice: A Survey of Applications in Vulnerability, Malware, Firmware, and Protocol Analysis \- arXiv, [https://arxiv.org/html/2508.06643](https://arxiv.org/html/2508.06643)  
33. SCALING SYMBOLIC EXECUTION FOR EFFICIENT SECURITY VERIFICATION OF HARDWARE Kaki Ryan A dissertation submitted to the faculty at \- Carolina Digital Repository, [https://cdr.lib.unc.edu/downloads/r207v414b?locale=en](https://cdr.lib.unc.edu/downloads/r207v414b?locale=en)  
34. Symbolic Execution in Static Code Analysis: A Game-Changer for Bug Detection, [https://www.in-com.com/blog/symbolic-execution-in-static-code-analysis-a-game-changer-for-bug-detection/](https://www.in-com.com/blog/symbolic-execution-in-static-code-analysis-a-game-changer-for-bug-detection/)  
35. Efficient DAG Scheduling with Resource-Aware Clustering for Heterogeneous Systems, [https://www.researchgate.net/publication/225217402\_Efficient\_DAG\_Scheduling\_with\_Resource-Aware\_Clustering\_for\_Heterogeneous\_Systems](https://www.researchgate.net/publication/225217402_Efficient_DAG_Scheduling_with_Resource-Aware_Clustering_for_Heterogeneous_Systems)  
36. Correctness and Completeness of Generalised Concurrent Model Synchronisation Based on Triple Graph Grammars \- CEUR-WS.org, [https://ceur-ws.org/Vol-1077/amt13\_submission\_2.pdf](https://ceur-ws.org/Vol-1077/amt13_submission_2.pdf)  
37. Concurrent Model Synchronization with Conflict Resolution Based on Triple Graph Grammars-Extended Version \- 2011 \- ORBilu, [https://orbilu.uni.lu/handle/10993/3922?locale=fr](https://orbilu.uni.lu/handle/10993/3922?locale=fr)  
38. DSPy: Compiling Declarative Language Model Calls into State-of-the-Art Pipelines, [https://hai.stanford.edu/research/dspy-compiling-declarative-language-model-calls-into-state-of-the-art-pipelines](https://hai.stanford.edu/research/dspy-compiling-declarative-language-model-calls-into-state-of-the-art-pipelines)  
39. Prompt Optimization with DSPy \- Haystack, [https://haystack.deepset.ai/cookbook/prompt\_optimization\_with\_dspy](https://haystack.deepset.ai/cookbook/prompt_optimization_with_dspy)  
40. Bayesian Optimization for Anything (BOA) \- OSTI, [https://www.osti.gov/servlets/purl/2437681](https://www.osti.gov/servlets/purl/2437681)  
41. Trust Region Bayesian Optimization of Annealing Schedules on a Quantum Annealer \- arXiv, [https://arxiv.org/html/2510.15245v1](https://arxiv.org/html/2510.15245v1)  
42. Efficient Development of Consistent Projectional Editors using Grammar Cells, [https://www.mathematik.uni-marburg.de/\~seba/publications/grammar-cells.pdf](https://www.mathematik.uni-marburg.de/~seba/publications/grammar-cells.pdf)  
43. Triple Graph Grammars: Concepts, Extensions, Implementations, and Application Scenarios, [https://web.cs.upb.de/archive/fujaba/uploads/tx\_sibibtex/tr-ri-07-284.pdf](https://web.cs.upb.de/archive/fujaba/uploads/tx_sibibtex/tr-ri-07-284.pdf)  
44. Model Synchronization Based on Triple Graph Grammars \- Yingfei Xiong, [https://xiongyingfei.github.io/papers/Sosym12.pdf](https://xiongyingfei.github.io/papers/Sosym12.pdf)  
45. Autonomous Incident Resolution at Hyperscale: An Agentic AI Architecture for Network Operations \- arXiv, [https://arxiv.org/html/2606.09122v1](https://arxiv.org/html/2606.09122v1)

[image1]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAA8AAAAXCAYAAADUUxW8AAABQklEQVR4XoVTMU5DMQy1xcSCxMTYDQQLAxJn4A49AmLsztKlI2MF6oE4ABJTxcKEOnVhoH2Ok9iJ88WT7NjPdvKSr08k4OQdAqGYoDtes+R9ocTDTbgZG/YO5yZRN9TYhWeWKW5gK5TWWNdoeEPXF+JqbPGvjuiOl7CDGucVxoVrbAt7KpP38Dusp5I4XMH2qN2lLF6YpfANW6TMFRBvsL4aEx9R1pOU+J2ZzuHfYfNCmK+IWjJE0R52Wxl3sqHXopAT5XFEQcXw5FY2rsu0YX1tgzskh1EIiWROkn+MajVb2PJCzEEcWB+scBY2WdxUPpEMvxidK/+gfKI/9D5EVYqGc8mS9JWfjRKMtshIJaYL+A/YJ2wWbtndrU2YruEfqfvdaqObjzoik6B0e3LGlJY+jZMKp2WqJSHWq6iAwLlDBEd0siyuweQTHgAAAABJRU5ErkJggg==>

[image2]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAsAAAAYCAYAAAAs7gcTAAABSUlEQVR4XnVTv0rDQQxOQMFiobi76As4+AAunbuLL+HkG7g4Ooq77q7ORfAB7OJSCm4OopNW/ZJc7pL7acp3+fL3lztSIgjjZzoK9w4XDwyLtNGgSzmtJkSrM4kPY9wNroU1WskUuACuHIgsBahaevIIxxnwA75GcIX6Fey1aUYBLyRzs3R5lqpSLTIDPvt5p3B8QZ+qJTMyTcAewJ4sxS9AtIVjHFxyVZn9G7h0b7teLVSyAzzC+QK9nyJNxFTXAfAGeg+9nVKyaPIJ+uBV6Lx1C32VNvuaNJlndtnyxThH4TYvsbzzXs5IwtLkEPoDKTfgGyH0l7C86x0w0mJz3gJzYCKOY2q7IG+LZPa9ELyCH1kd9kFvz7IXuhumzX5H8m7Y6TBQubjR5q9s+Jdy1uvCo2mu5uxjJimYUwZfVdV98d9REuly3PwFywwyVTpjPZUAAAAASUVORK5CYII=>

[image3]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAsAAAAXCAYAAADduLXGAAABMklEQVR4Xo1SPUqDQRDdIQaElKks0qTIBQKewTpF8AaClRhIIKdILWmsBA8glnaCtYU3SGmthfHN336zkwgOzOy8mTdvd7/9ChU2jf+2OhTmPG0Fs3DEB3kms2njtBANBTnRMrcefA7/gO/NF9oyng7ShKi8AbwDX6NwaeR79JToCeIOyxI79XRHqTJ5ZUSJffgjfJQuMIVvSipeAP7EgtkNeIPIPYE/AH8y4HKUERyOO0ZlB/ySdquTQrYLzEhvvHVCGmkK/Fn2JDfOB+gyQcRkVV57Jys7kQ3KTKbnTMD37iM8IR14/dbO/NUwNS4Rz+tupj6DfxcZkhfj9RXrJHDc+FeiMyR38CvgUezoegxaRvUlgtVS/ZkiqRWwRLS6jKzpDAoSlXxctHnurmr211F+AUxkJLnTAfQcAAAAAElFTkSuQmCC>

[image4]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAA4AAAAXCAYAAAA7kX6CAAABdklEQVR4Xn1TLUuEQRCeQYS7JGIQm4LFJpg0XzGYrDaLRQRtcsFiMRoM/grLNf+BP0CDIoLYFBQMhvP1mY99d3bvXh9uduaZr53dfY8ogCNxHn2shJOvCAU7gj00oXOPepcITUtB0yUTnXp1Ik+sugcZQC4g15AHBG7cvkLGniU7fIRLqDGkAfsl0x8INCqsXGQIe9YKidZJu/IypA8+sgbtPHPQ++CfXnwgZcdYHi2uWILvBfor+PLxrfC28Dg2MPs39FNyVBkN+FkOtFHekSD4yHks7EPeIKvOU0j1KVnX8+QPhXKru5kmMPeQNCK7wW2p8KIZGEfQcjnMUx55AJ88hZzvGTImvWHeIilOqOt8PNntEPYiheQ6N2Iecgd5h6xVMUN5i2LrYTZh/ZCcUT+97j1CRM176FeYK8btGdJ/0H8xP5ksl3KSvXXfadq6LrSptqEuxfN37fo/pMoqy5Jiu9qKrBinRvSUo06SdoS8TjZMYPoD5ys0YLS7YXgAAAAASUVORK5CYII=>

[image5]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAsAAAAYCAYAAAAs7gcTAAABUElEQVR4XpWTv0oEMRDGk0I4URAEEcTGwkIfwOMsLMXGSjsbO5vjWkvfwE4Em7O+1hew1pewEER7BYvz/E0mk8zuXqEfO/+/yc4mmxAMMT1JSsoHBSmpldQAay6vCSXpin+gJ2QerT3UOt5C7XfDyejIAfJEOMvyRmGnkDJxCRnjvmBHtJ3iT/FnyHGDDO7oeMauWYLXPKJfcbcKK88pK1z6HM8NardylNgTMnJRv6AW54X6QTF8kbvHnkjNLZihmRHb9YMvDdJ421o42s6Jfkddk9jGX64MBwLhD1H7pWQMm8GdRZ/g03Pay1m4iDwEOaUOmgMLOJko+/uRIqkrR/Q5es8CGWWA+Q5p2+IRskphg/gq6MnpyM4OkWlusG2bIJvlw1o3QX7FM+whdkUJrtodv5sx1IrN5GFvzyrfoVb1P/C95jcvUrH5P7F313Qyv12IKg7ROV96AAAAAElFTkSuQmCC>

[image6]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAwAAAAWCAYAAAD0OH0aAAABXElEQVR4Xm2SPUuDMRDHE6pQqSBaUISCs9DFxc2xg4OLCg5udRE/gH4Cl04iSMHBtSCCu5OjIDi5+gkExVGKL7/Lyz3J8+ToL5f87y65J6kxxrqfs+jVcqEWTpe2HnQWtPIuqiYNVEpjpVqL6TJ+NQq5VbUdpq/4v4QXWNOMkG0ZD3CfzE/w+/hDtAfCUrTjE33uKcOU6Z6WV0ceBzL7gQsTyyt9Az4QZsPahRZCG/3CFcxDV1ehpUuGc80z8R20fA6240J2eIQz/RxNdL4N97Ab1RV4M1IQU3Lrg7S7HoVFeIY7mImiNyvrsfXv0E43GxgvfsERiT2CY+ZT0a0/ITndfaOVaw2vasVfwRB+YaK54ZbEunh5SXndTevbm6DJRoNs+6Y5bQu+4Rrco4VM22K4MdKK/78I73BLbEkTtRFvssMI5QkvjHy8dLqKpXgilMKpNYJZQXJIXarbP5C6M0TlSRABAAAAAElFTkSuQmCC>

[image7]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAA0AAAAWCAYAAAAb+hYkAAABVElEQVR4Xo1SvUpDUQxOwILChVoKrs6dlT6DOnYSBFfBF3DwHXwDZ6EqOHZ0soODo4/QybGCDr1+5yT3JLk9gynfSfKd/J30EhNRQhJvd370WEkX6ZO9FiewnH81MVasSpFecswY4zgwThOKU8bIcgt8g27BttCNxLLUD6EiE7BL8PewV7Bb4EiuXGW1BsAc3o+7mlHqJHckSWEkPsGxAfOgoySywblvIXGuPTALnf/c6FTYvLI5lRuShCcwOyVGz9pfgy60AJ8efBVLe+m6yv0E6gtYA7olS80TWZ1iXbBs6B0Y2Si9jraEbHxCranSRSbKZc5gNX4RG+gP6Bm8Q+jdLkdlCrwCTVdtzLKANJ7HHXBKUuQZOJZwpiEzP8K6hHON5Bdwv1Ygfz7Jfsvh/okyZe/RWfxC8uMqF1uJW6WT0uVWg2vfgIokxZpeAq9T/lOs8x8e3Cw6BReeJwAAAABJRU5ErkJggg==>

[image8]: <data:image/png;base64,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>

[image9]: <data:image/png;base64,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>

[image10]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAF0AAAAZCAYAAABTuCK5AAAFCElEQVR4Xr2ZTchWRRTHz2BKkRH2YQqBCC2KChGhCGwTKrUopIKCNkELIyQoKXcRRIsWUWAShF/vQqIIWkkhEQ+0UAikRRGILYKgRZgUFESknf+cmefOnDlz79zn6wfnvc8958zMuXPn48x9iRaN04o2mos1O87HippZMSOfynK3dMtgae20VNzikxNL6OvMrGO5NVwXQj0iWFzpkN/jbhMl8Wj3JlQhVLaZZWuuXj5G8OtZjrL+NYLZcFgBd5D0xcZwjygOspxmuTk65SSBVmMWw40s37NcS+QCy7apH6hWYjPS3ZOU+ZDlOEnnRxPuj7I8GO416Ih3WF4heaYOy9vC+YH3CMs5yvvj0NSD6C2WL8L9aFDBMyxXWF5ieZrlOZazJA09Hp3mpVpH1UA/sdyV3GMG/kdlJ6Qc4Aqj/Zg2NrCF5SuWSywvs7zK8gdJfZMkVix556kvehNHr/Pff1me0ibmxSCjGBlBR1kQU/g+rQyO6Pw4Ky0wMz5i+VMbaoTmb2C5yvJoYsJsOcIO6CO9r2Dp+YCs6KmuxKh5X35m7GT5nbpprRy0+8KXW6yfE5brlT7lMEmnY1OzwAz9AT8ktqYId5AMwt3akJPV9QvL9lTRB9a+K84cTf6hMX0GaXoUzXChXSx/aaUCHcMd5O7RBpIWsOZiFI5hl5MX+TXLLdpYAf5PaGXtITGFzrCtbzTVSeq0q58L7CnXYsWV+pFR/EzygjyJ370k+8FDnaoJDLa4H/zNcorlttShY9oaBsfbiaFKnL6YohZ4EZ9zvU9qgwIB7SfZfG1xuLpUd38o28cay69aqYjP4Df6BOxTP5LOugyMl4n1+7KTOlA+polkegtYwiaU+dpgA8BI6Do91ilXLDlYeqZTt9qkxZDzgN1Jp2MU92F1OmrGyJsmBgNNpWC5RdZypzYMMPHiVKcbDW9i5bd8/YzluqgMfrhHfowpNtvSk2G03g8yhE9ofKejoeedjNLhRnMPJAwnydxHOsdKpZMggyMdHYsORyM4ZER84NTlwgVlw9CU2pR+q8maa+90bH5o4iBJ5uEzruGoMrCMIlVEx4/lG2rsdMbtJelYJP4vcISYVhjhCBx6HJbqyBPtIQk2bD7TQ0mfHPElM4ruWaPhHBupImYrNss3Wa46iX8smM1nSGKz9jgcHLFuP6ANAQwOLEv+9CtPUjxPBtJCTE9scKgUM+BjkpGOl7JA+gNRPEaVmRZxst9cDi8aS4o+uKTgZBpfukaShs6OXB3rOwbhGySn9BC8+QwYpAe0MsUslfAwyz8kp7n1Q87z0Vv7dpJDR0fpHvJ0nyMra+GMAYQZjVkZyHzwqSGecFP5NHWqgBUBiUeFPBZMB+SXaDBaviNZn273d15bPECFRr+Km1LHGZflyMrnWZLUcAzI/2uscz7nd/DZR+kXxNhwGTtmyZcknw8GwVQ8Qflb/c3Jm209iWWU8cxCVgtGZ/oNRLPBJZ8pGkDl72llE0lY6jkxwvHyc8zOcBV9RTuKBVSRgCmPjSzHtzGqIYxabJbGcX1mkPXhrDOOUWEvgXr7sHgrNveL4WpSryMDS+nd1Ow+yDaWC05SzVVgxG2oWmgshiwF/7S4SRsspnWWP6oMe2RsYHmX5J8c4wp73zEF+mipp8WnxjxlG5mlia7Mwj9tV3ALaMaVwer7mWispNFtSp9/aYua0hKAoWoUDLOhWg4ra2h5/A8kg8IwaPmhPgAAAABJRU5ErkJggg==>

[image11]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADcAAAAaCAYAAAAT6cSuAAADP0lEQVR4Xs1WT4hPURQ+N9T4E4YpaWajpGykLGwmW2wsjCKhWbFhg0hJI1nZyUJSGiWkZEOxsrdhoaRsNCs1OzYkzvfOve/+O/e9+34mzVffe++e851z73n3vnsf0XKCaS+hYSRURcYiLUSzWVhXpFDkRjMuCdq8aQeFslJZhl5BgCFai6qQKlEFOvKoLtXosWSzWE5T9qToVnZ7K7Ce+ZR5nLk68TlsYJ6msv8fBtEZ6pyKyCi2Fq3vIvOP5T3mKucIcIbEf4P0jlLDqBghkQ3piJxl/mYBF2BOpE7GSZLiPjInskRpuwaVMVhWt5iLzBcchLd/nXnEclLNkxv3MH8wF3NXg8PMl8wxLRi2TRRPO1TjfF0Rm/yzlibBO+YV5lpv8lFVGQTjJLkwQxruMC+nRocp5gfmd+Z+a7tKkmzOtodiM3MmNeroKVLewiNqxmO1cQiW5PbIYoFd5jFJgQvM+cZq6CFJcdIejrMUDKFn+IJu0W3SZw67ZWEzITrI3Md8wO5ffJ8OZCgMu5HDLvadI7d8nS4vYSvzGTdxL3ELpbufOry2Eyy7sDg4jpJ8iz0wzaxhXWN9AyuZmL3pVkN0n/mGuS6wBYiK+8z8Wqb5xPe9LiCsTK1RvqmgODNjJ+OSt5WBwJvy2KTfxnxOvtghwKy8xkP73pMRFwpo0PhyQVgcdsefVD77MiAQS5Rs5mN8u+bdeW8pAgVmfd43Bf0ZLHQhVg3GOGtw7jVtU/oroTQJAg/ZZ2z/5wMfZnGO+YXaF6AhSojGK77giOmHXpADPoO3JGO8S+mvVndsgyckRwH+5fDthdO9W2iw5WLJeSSJk+YpNizwksR3MWXkpWWoGNsEyXaP4qqWYQzpYSPJZtBuGEHHWGo4LL2pfXIP6jBx3l1gvmd+Ixmg44FA14Ud1GxQxq2sAGqfOUQWiOO4nSQ7Jw7LuqW2dMCMrynVEZkLGg9dgPNukuTAxMGpQ4mNTYrA2bKV0If+zBk6RGO4dPgHoStP5ssMQ9Hz5mAd8LP7/7F8huZeU8WIKiSCaiEQiAfFOYwUNCIq+wplzXNPXKbPWt7aP1slX8k+FG2epUhYymHtJXeK8hvM2xZ/AZa0YyOY+8T5AAAAAElFTkSuQmCC>

[image12]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADYAAAAaCAYAAAD8K6+QAAAD3ElEQVR4XrVXTYhOURh+TygyojBMQ4xsRDaTshg2KLMYYSjFXknUKMpqFmwlLCQbSpOfspASyhSJLFhQSgopWbAbJY3xPPfcc+85555z7r3zmad5vm/O+77n5/05772fyLShfEEFtRYxA8pjOgtxk7imQAOTHKqF7QzDP4g/dpBUNsJCX2DQcuk5vqAhWm4TRVFP/DgKXnTUUdRm/q4kgkSE5m8Fr4HvwTfgHletYU08C37JSfvVWuwsPYzhK3x3h7f0UG9yEnwkFefSE1eB+8Bz4BRs6WQcSn4K7USegwekWiZrwY/gDk/uoXqoqqTAYvCFaAcTZkRRNQWYiQnwMTjf0ZRYAT7BPARAHfSVOc6DD8B5viKJmuOKDuJ3cD0H9eYaPPA4+A78Bq5xtBpbwDPgH/ApuMAovBjF5ncKbnFJ9BkbYzs4Jto5Zq3f0eraviI6ACzDC4XGDd1cSWeceh9dviCOrEoY2MY4DZ4S3UR48CFXLcfA/eBscAq+7C09cjzrweiqLcigm94R8C14CyPjDO/3V7DPmFbgBm4T+MuRJMAo3sYCA6LvGRxTI6pckDXNEmCT6IXuB77XFVoXzDQD5ION5Iboe2dXxGbsW4wb3BuWOEu9gHcNHJgyZDaYKWaMh6A9s8RsGfDQZRlWwflutpUsEr1GF/7vB++I3ssYPMPHknIsLLdt1thGD/hZwiVdgSlDgpGbwObjomuf9yp/digudl+cMsw15b9Vx1yMij64Pee60NEy9B/AE6XagiocY5CS0GUoMpDZqSLVnLxSdCc06IXNJ4mXIQGn1FB1z0zCQI1LtWPqx0Z1Ugi5Y9ZFicC0+d58zJJgO+U9GhF3O5bsX4iKNh8A7ynnuefUAwaE6+qOqWWshr6oV1WxyVgJY2PZshng7UE9xPeyUpx1xklwlyXDk1/xlYb3b5Yl98HAmPvpwwTNdES+ofAd0IBvP7sxk6Von8cGgzsZXD2HiR4PamiaAu8b7xYdH/RsDLOsFMgqI9uNHzy83QxscL3X4E3wpbhljbubzWMHjh2dZ3O6IlH7Cv2f8Ft0ScbAbHTjLHnmlfkj+sCdxtCDaV5jjd2wPa67ljVqgg/hUV+o58VmF/LD4FIMN4rzSMiwQfS7YtVxf1l/nCEotFFrcEj4k0Yl3ibCMBmhA8c9HTfly/U9sV6ukydJKnNkNrWGVtZFluOLTYm/y+rhTA02Jzo87AtrYdatnoGSuLYGdO6yL5wmBmP7J69O064StHJWDlmEZSFpO9SuEDJQYfEMo/MtO1khmfs0Ws1sZZyj3Zx21kE0XqJBiSYM/gGPdYhN5No/jwAAAABJRU5ErkJggg==>

[image13]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAFQAAAAaCAYAAAApOXvdAAAFaklEQVR4Xs2YX6hXRRDHZyFBySwtELmBloGFUYGI/fPJiiKKyKDAoIcge/FFMUFBjehVQqMketCHqCgqiAoi6hdBREE9RVAE90YhPUgUFP2hdL5ndn9nd3Z2z7nn/CQ/MNx7Z3ZnZ2d3Z/dcosE4rSBbF6jZCgzoEujbtW+7PszS13jGRDOmb4Kr+xpq60fZQ8Gyig0FExU79WJo36if5YJ1S/nHcq23W8c09q5Go7iYZbtWDuWcRppyOcsHLHdrQxd7WV5QciHJ6hwybI9IN7pB6Y+zrFcTRjLfIzMPztRW6dW+OQrX8C/PkcSFpPzA8pb/G3KU5Q6WJR1Or2L5VitrwNvNLAssZ1i+Ytnh9TwYbWPZ520nWR5gWdv0JJpjeYwkWAR4F8sybwPw8QTLZ5FuNlg5SHWXsdzHso/V//DPgySxB/mIZE5fkOzEGg9TO+ceSCATapOmWcNtYLtHG0iS/opWejay/MzykDYIVlZmDhYZsc+lozn8+ay3vZyYcnDKXmS5QBtqnPRJe4dlaTQ4nDxPMjB2asylLG+TJE6D3TtPsnO7OTe5RdnCcUfsGszrdRLb98pmcZrlsFYaTGeyn8T5hGV5NL+tJDUQtqdbdQMSrJMcuJekz/3a4FlFsrtjLuF4cLN2028BrmQ5RXLkNXPsY95JjNipUxrXuf9PyOdG6YuEOskd3UVeh84o5LfA5tJygF2J3YldaoHk/8ZyvTYw61i+ZPmD2oRjAf4l8RnX4TGERbV2IGo7bLhw+tRHzP2Uk0Xy5FmPcDv8kV8g1EzhcZYnYSRJ6Ev+d4AbHQFHuNbqmuPEvlzwFUA9eoOai8D9SHIkcTT3kEwwHn8sWFT4RCwtrokBi/cpSRyJuQA2HDbAZm0o4ELx/p1lE8sRktstMC0HJLtyYxyHEdLEiz4ij5I8t3AR/MdyZ2TDbsXkQ/G/jv3uorg0RGvWAZ5NqHsY4zZlo4IXvGjCC0eDCxk5sC5mEyQRyQwJxYTjoxeOzmqSnWsMmqjwVJpQntAAduc3JP4CuibjZn2fyj5S0oiQGMQ8T3JBtrSnKGYFyY7FBWzd5oMTik545mxJzc2Rx3HcyoHgkrJpg5x4aZORTgDjxC+AZWx+lSQOhbF23WAxwnG3ElQnHxKnBzmIEho1yts3dQsJQxC4GHQT6H8hSdKDqclkQu2OtmB/Ln4B3EQy+VldSHiwI+adQaEnFFOzeZpL28m7NiHvK5qV1AYR185AXA76sNvJc+VWbfBgHBxLgAiOUVsrr2DVWpIF0RMIO6/wsdCApxfa/MSyQdk0GBtfU6i531F5fljsr0m+wDx5KiMcjuaEdM1q+6DAP5VoIgwldtxfFO0QBb6u/mR5jeVjdoCbPoBLCwnDBFYr77ez/Er2+xdPNDzVkMxYoCvBb9/mYuQXi8MCpvUWyPDzJO9VY6o2qDNwvL5VJX1RO6JJd/rF0wQXU/PlpWwBTAalBj81uBSnEzBGsxI6RdqnvQwfMXhiJbGq9n9T+iIZTkcgNfAawM6+NiiKvnLDAkm5wH978FUVg9bGU8gmd+1JDXgP7y80hhabA5tkJPYA3Ug/vFc/Z3kmsVnE48jvqOc4flxmnJ4IvtK8blyAEVjA0iJhPPUR8/+ynfp93gkyV9Rxq1Sg7FytlTZZ0mpYzys4wCfqu9O/0l8KQzRKsZj2TkKvam/UKB+w3S7TZooOCu0r6gMsJ0hOAr7gErjBjYS3cnph2hQG6YHV09L1J+udKWr0WkwL7P43WT7kvnh36/98tRRdFw0lFt3h/KM+BSQVr4x+n7YZrsv/DFjsALX2NdsYan4H5Gix7ctknjLF+cmgMNNOJRdnAdhI45pQ/3tVAAAAAElFTkSuQmCC>

[image14]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABUAAAAaCAYAAABYQRdDAAAB50lEQVR4XpVUPS+EQRCeiagoXAiVRiMKVBcNvYu4Qq4TiU4l/oLGHxCNr6h0avXVJKJRqYjQSUgoSPDszn7N3r5358k9+zHzzOzsvrtHBDCbxoxSRINy2YlY8pB8blE0enin64vaTFMJzhR2VzKqXiAP6h/uCOIqOVxqpgbaI/AU2kf0t2bsbCDvoB8NYf0AiWfQtMBzTB/ATbDlCf8X+g9wJUaRFF1ZcMQ+NCepUnbJy2h+wHtwUhxB0hVT4As4nzsQv4b2F8NvcFF5eqAJiQmsdUiZDowPvCJztkrgJuHD6eg9kqQlPJOc66qdhbiO5SPgGkbXJpc0XjceQLMEvoGNLikcUgVTkyThXe6yMAZtHAS3wTmZKkEY+K1fKKuCsi6Ar6ApRvvcMN36VmKn5G2qQsqLavir9AnWY3QMLSfJdErEdExS5SVJ1QpGa/USNA7ukt0Rt9noWS9vLvk7mYRsk3oOBY1FrAS/WTJPmujMsfMssiqyLZQQBNfkzl9B3IUs/uMUXL5ckj+deuaJg87YHmWL2dznMe1IUREryPdjRzVwPZi83bqyZF1zyx+0uR0bJM92WrsTpImKSZ2RJckTeAjeJIqIkKCUyX0otXHpRtBNkLz7KhT2X1rEosqR2KskRfxD/Ad+bjsWaSThggAAAABJRU5ErkJggg==>

[image15]: <data:image/png;base64,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>

[image16]: <data:image/png;base64,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>

[image17]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADsAAAAaCAYAAAAJ1SQgAAAEQElEQVR4XsVXTYiWVRQ+FxMUDRNECQacmSKIhlFQJAejRWbNohYpJLQJIgQpg8QCVx+IqwgGaRZFUCoulCAiolmIDBSRuHDjEESzEIQoSDGwRT9Oz/Oe9+fe85735xtm8IFn3rnnnHvuPfeee+79RJoQrMCi06AZrV1blQ8Sqz+xUBvDtlcCq+EzBUfYbIUJQn0SdUnmY40V9kTpbh34KPhIpHo4NojhCjO4mrUQf4DviQb9MDgBF7NCn7I8byPg3+At8Db4C/gsXF3A9yHwOfCTHjwD7pAUnM974Ffgeqtoh2vBIIvxsoCHwW7wd3BD3g7Iphdy2buiE5wDl6Qa5HPwDngFfA18A/w4t2E7xjPgIviUkXeDsQYn5CCPi/o8aFUNyErBCP78hMaM1QIDcB+4C1yE7VQ06Dj4a66PwYyIZVyor8GPJB+wN7psQxjg7w1wq9G44Hn8TjR1GXQCzOwlfDYylaGaSLVyWnQXmeIxvgW3RG0GeU1aC5MflS9NgIUMc7D7xio8sBjdFJ3066kqw5PgE+Cnkp41LIDMi/ZLZxXkvGihK9oLov2bUNlWoH8f9RXgojPDOsGBuCqcNHkcfAweu8p6kcL/WEWFclb3RM+9BQ2OiqbhJakC3C6aaWN5ux1BXsHf+1WzvhoxXgb/C1XA5F3w1bRn4oR9aMddq6Mw1S/teBwiZIrnQVZ6ZgwXhHWBeFrbIWu7U0+FrA/5oucKt1OETB9kFN93RAdbCn76xSl8JFbkPiziQArwHj8m6ou6LyQ9+99Ldu5LZ2+Dk55z0f4cI4drk8E7L8RbosGctQrRc/yH6GraSuzBBFubzECKq6pSnZM0+N9Es8mDCbYZJr2k2B7KGSwfCLGCiFM4rrpN8Ha2QJEl42YR7D1damtLlfkOncHy2qmCTb1gsMBD/2IszE24AAg22NRLELnjwkw7kySKLCkeM8Qm9B6L2g4Sb807G5mxivEFNKXSTMNBeXX8LFoVLfg0o+N/RReqq2oTDOZk2YomgFcaM2MBsqIS81X0ZWWR3Q4M5rKkCxKDdeNPK7TgDr0J/iV66fMJyOfhD+BoZZaBhUordTBfkf2JZR3zotdbWR/MLk9DcB3fi+BV0d0maM8N2SZaQzzQ1Wei8y8FFtyRvfn/XLEDoudkVEp7r1s3nF7vi97JvJtryHOKAfHJ52UK5znhetaawdoxMHIfxVXq/IxsgGMYUmlqEfj4ZzU9nIj7gxnI+jJpFaI1hb7tU9abZYNwCPToT5NT4I+SFZ9mOL6KneMvsEOFMLcrfmDMmLXuiSG6NJs2angWeSb3WEWGslutP1PbClk8+fCIfFkTB50mbQZtOh8MeLZqWgdx29fl0g/BnVG7GZVBl2mX3kFLF1+VSn0bg15Gq4llTcDp5IhWFz0G7Kzebfo2XQ2ecS6LVZ6ZRR8bi6H7lB3qPeuSlcX/twSiHe6lsSgAAAAASUVORK5CYII=>

[image18]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADwAAAAaCAYAAADrCT9ZAAAD70lEQVR4Xs1XT4hOURQ/NxQZGf8TZcZCRCFZqFlaTDESFlMzyY6FFUVZaKbY2JBISWEhxSgWUlgoG2UhZVBMmVIWslEUE+P87nn3++499777ve8P+dVv5t1zzj33nHfPPfd9RP8CJvlYgtCisX1jOB/N+WrO2kPLE/NIuY1kkaCAlaeUKZlDTlcONaubOTsUNQvrcX79sV14TuSxI16BPcx75IJtB4Z28N+VWlyCIIFNzMsVec0fm1jv8xTFeMns0cI2AH+rtDANl7OxE/YxTzKnmeeLseMx5otCB+LZJYUFIfvMPMEcZd5m/mR+oQAG6wyHomDUCm6RxDFLK6q4Rrl9pGSZGDh8QAjYBL7GSBK+CiNPfoA57o2hO0e6lFNRRccpi23MrySxN4UFzOfM41pRAE0GuxmAA/pNspPrlGqIpFIc4FfteMdwkPmDJPkyRO9uI8mb6lPy08X/Ncw3vqIAdvcpc56SI0EEAnQxn/CasEsB1bOQwqBmMBd5Y+8ERtjK/E61zQotEvYW2BEEv8KTzWFu98Ya2HV35sthaDX//cS8olWMnczXzPfMV4VsJvMSc7osWIXlzEnmmG9fPlfOJM4ggh9kARoVXsBDJoItA3SYs0sr1HJ9PJpiWf241NV3SM41ziB8AagIrO3GFuUJuAqylVYJW5jf2OWUJ4P/G944BezsY+ZcrVAYIAl+QIKuhY4PkG4e9ZI0S//I6PUxHspkfZ1klzOoT8ZZ44CM31VRVje9cYHaJPdW3RnPYdAECUfgqrL6C54M/u9642XMd95Yo0LCAiTmXy2+PNH1jMuZu7JB143OeJSUqe+wVlF9fXR78SUONpCUe4jIuYWrhsm02sKpDJrUB5KAcG6rwjW5xJ0dwSV8RCtIdm6CUNIm8DXCPOyNa5DIg9S6jFTbREIX4QxJMOiQPaFKQfzguljLfEsyT34E1DY+uZxLCjtJygIN6xlJF0cTxM1wkbm30HMDNbtJGmOipK0vdwtkbwt37yJonzi3KLNU4HCo7R2RjJ0Xwnpx/WA84RPAtzyupUmSr7P9VF8e3XsxyXz/jPvAy/jF7K+ywx1HyXJoTAgqQmGPyllKyZ+NppekQvq1xsLYjUCHRyVZgadrFw08lKuRDI5NzaLc1OlqFrhF8Nm7hOIqQpJIdkTJO4hcpJRVD7NuvRbmYGTH75Ncf+jc2v8hFuAYVGmeLaJqxVilb2GfH5H83FRTU56CYsCPG5S9DzS8o6QMy5HXtonSt4KvsrPMzR1YPnpxHfCp8Rdc/ldoIb+qUyK7SJCDMw4npVykZFnkJuR0eWWBKjYdQ4PFwnfYwLgiGntpbBGhwpQ/JvOq73F5W9IAAAAASUVORK5CYII=>

[image19]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADUAAAAaCAYAAAAXHBSTAAADu0lEQVR4Xr1XTYhOURh+b0zRYJqUoab8LNTEgpAUk8VYKE2aKImyIz8pSs1uCguWigVKFlIspUZNGmyEkiIlFqMkC5QyNYnxPPe9P+f33u9+Mzz1zJn7/p3znp/3nE/kHyJxBR7qLWYFTbsJ2YdkjRANEFUUqLAIqAKioExkQcYgwi4+InYdrqABrJAc4DKww+ko0EGyEX8ewW65q2mESEbAGNhrq4Nf0QjnwE/gNPgNnATPgPNFB91HI8ObCb0DN5ciC/3gVfAp+BH8kH3nvAjOKazDOAC+kjYmjYO+AE6Ah8CubOCLwFvgA9EBdau4AGfxLJh406QCDoRxOUk/wRNQ7EFLHgVfw+wu2q7SxQN3xx3RSfB3SulkuS8FH0PGjpmci8XgM9GBWUh09le68hRlF6dEfcfFP3fsj7oRR+5iC/gDHPIS9wQi90SDciasWXBsh8Cv+UemG0Q7YAoslDKuMPsYLCQluPLUjcG+01UW0FiHwanqrhTTkHxHu9aRu2BS48Y349wAe/2QHn6Bn8FVrgLYIJrUcVcRwCZwsq63HtGkbqOd6ypzZEFYCPqNBDjDz8H47JbQlXBsE90ZPCfUpWeqBqzGE5KONZ7avuwc+dsiKd0i7px1zr4iYiQ6WezjvGPECb0C/pHWEiJ4HsfBhbnA7XYeeB98IoaRB9erFHDbsJrVYQAuPIt9gWAhMMFeV2jgpuiKBZFnTRYVyetWBUfAY7YirUZT+q/npVAxVihY9Xyo/V5wvSMz/61MirguVod+BNH7ahQCrowJb6VSFzs/niGeFyYmrtJD0opFemdWJoUzJW/BHg0WDMnXxGUpCklh4yXlISnOnZb9IIJ9xpDvLp5HC24UlnMObj9U5nOlE4bDsObFHAJWIeEqdEdWmAO4JlokvEE4WAOeBPkquWRp7NBGcQr2WUhWQ5i/FpggSmzyEO0X4ZPGdzAxItlb0MBO0VgueQHHktsmuqXylYiBVfq3K4yBM7QC7W7RNxkHWvfIJFgseMtbqJyHgNIQcSUmys/Etecq8ri0hkBfBTxdKeC7bTRrCxTqCscIuMpGUha4ykxoxBTWRizRwFS37I7CpZGrwnBhleTd6SNJr5U3Yt5hdX2pvs4qiPypE/5Z0DISbr33GMHWwDD4s+e0tDnAluFE74BgGO0uS9rSEFCotFDwh+BL0Z85Lg5K1a4uYauiu8cTzDq2S1oAkhdo1zm6AJKZ7Kq23ZqC3SyR1qrt/8DM0q6f8Spdm6gPWW9RBXvLzywW8ReF9IdfGoUP+QAAAABJRU5ErkJggg==>