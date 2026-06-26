def task(name, priority, done):
    return {"name": name, "priority": priority, "done": done}


tasks = [task("Write report", 2, False), task("Fix bug", 1, False),
         task("Email client", 3, True), task("Deploy release", 1, False)]
done = len([t for t in tasks if t["done"]])
print("Task manager")
print(f"Total tasks: {len(tasks)}")
print(f"Completed: {done}")
print(f"Pending: {len(tasks) - done}")
print("Pending by priority:")
for p in range(1, 4):
    for t in tasks:
        if t["priority"] == p and not t["done"]:
            print(f"  [P{p}] {t['name']}")
