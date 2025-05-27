# Transactions in the MOO Database

## Introduction

mooR introduces a major difference from classic LambdaMOO and ToastStunt: **transactions**. If you're coming from those servers, this is a fundamental change in how the database works. If you're new to MOO entirely, this is one of mooR's key advantages.

In LambdaMOO and ToastStunt, commands run one at a time in a strict sequence—when one player types a command, everyone else has to wait for it to finish before their commands can start. This creates lag spikes and limits how many players can be active simultaneously.

mooR uses transactions to allow multiple commands to run at the same time safely, dramatically improving performance for busy servers while keeping the database consistent.

## What are transactions?

Think of a transaction like a shopping cart at an online store. You can add items, remove items, and change quantities, but nothing actually happens to your account or inventory until you hit "checkout." If something goes wrong (your credit card is declined, an item goes out of stock), the whole purchase gets cancelled and you're back where you started.

MOO transactions work the same way. When you type a command like `drop sword`, all the changes that command makes (moving the sword, updating your inventory, calling verbs) happen in a "shopping cart" that only becomes real when the command finishes successfully. If something goes wrong, all the changes get cancelled.

## Why mooR uses transactions

In classic LambdaMOO and ToastStunt, every single command takes turns modifying the database. If one command takes too long, everyone else has to wait—this causes what's known as a "lag spike." It's like having only one cashier at a busy store: everyone has to wait in line.

mooR introduces a transactional model that lets multiple commands work at the same time, like having multiple cashiers. Each command runs in its own transaction, which automatically gets "committed" (finalized) when the command completes successfully.

**Trade-offs:**
- mooR might be slightly slower for a single user (a bit more overhead)
- But it's much faster when many users are online and active
- No more waiting in line behind slow commands!

## How transactions keep things consistent

mooR offers a consistent (serializable) isolation level, which is a fancy way of saying that even though multiple commands might be running at the same time, the end result looks like they happened one after another in some order.

**What this means for you:**
- Changes you make aren't visible to other players until your command finishes
- You won't see half-completed changes from other players' commands
- The database stays consistent even with lots of activity

If two commands try to change the same thing at the same time (like two players trying to pick up the same object), mooR detects this conflict and automatically retries one of the commands. You don't have to worry about this—it happens behind the scenes.

## When do transactions happen?

Every command you type starts a new transaction. When you type `look around` or `get sword`, the server:

1. **Starts a transaction** - like opening a shopping cart
2. **Runs your command** - all the verbs and database changes go into the cart
3. **Commits the transaction** - finalizes all changes at once when the command finishes

This means that when a user types a command like `pet the kitty`, the server starts a new transaction right after the command is parsed, and commits it when the command finishes executing. Any changes the command makes to the database stay "invisible" to other players until the whole command is done.

## What happens when commands conflict?

Sometimes two players try to do things that conflict—like both trying to pick up the same object at the exact same time. When this happens:

1. **mooR detects the conflict** when trying to finalize the transactions
2. **One command succeeds** and its changes become real
3. **The other command automatically retries** from the beginning
4. **Usually the retry succeeds** because the situation has changed

You don't have to do anything special—mooR handles this automatically. The retried command might behave differently the second time (maybe the object is gone now), but that's the correct behavior.

## Output and user interaction

When your command runs, any text it outputs (using functions like `notify()`) gets saved up and only sent to you when the command finishes successfully. This means:

- If a command gets retried due to conflicts, you won't see duplicate messages
- You only see the output from the final, successful run of the command
- Messages appear all at once when the command completes

This keeps the output clean and prevents confusing partial results from appearing on your screen.

> **Technical Details: Serializable Isolation**
>
> For those familiar with database systems, mooR implements **serializable** isolation (but not strict/strong serializable). This means:
>
> **What you get:**
> - Transactions appear to execute in some serial order, even though they run concurrently
> - No dirty reads, phantom reads, or other common concurrency anomalies
> - The final database state is consistent with some sequential execution of all transactions
>
> **What this means for your code:**
> - You can write MOO code as if transactions run one at a time
> - Conflicts are detected and resolved automatically through retries
> - You don't need to worry about most concurrency issues
>
> **Limitations:**
> - Real-time ordering isn't guaranteed (transactions might appear to execute in a different order than their actual wall-clock timing)
> - External side effects (like network calls) might happen in a different order than the final transaction commit order
>
> This design prioritizes performance and simplicity over strict real-time ordering, which is well-suited for MOO's interactive nature.