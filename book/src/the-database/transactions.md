# Transactions in the MOO Database

## Introduction

mooR introduces a major difference from classic LambdaMOO and ToastStunt: **transactions**. If you're coming from those
servers, this is a fundamental change in how the database works. If you're new to MOO entirely, this is one of mooR's
key advantages.

In LambdaMOO and ToastStunt, commands run one at a time in a strict sequence—when one player types a command, everyone
else has to wait for it to finish before their commands can start. This can create "lag spikes" and limits how many
players
can be active simultaneously.

mooR uses transactions to allow multiple commands to run at the same time safely, dramatically improving performance for
busy servers while keeping the database consistent.

## What are transactions?

Think of a transaction like a shopping cart at an online store. You can add items, remove items, and change quantities,
but nothing actually happens to your account or inventory until you hit "checkout." If something goes wrong (your credit
card is declined, an item goes out of stock), the whole purchase gets cancelled and you're back where you started.

Here's the key insight: if someone else buys all the apricot jam before you hit checkout, your purchase fails and you
have to start over. The store doesn't let you both buy the last jar—one of you succeeds, and the other gets told "sorry,
try again."

Our transactions work the same way. When you type a command like `get sword`, all the changes that command makes (moving
the sword to your inventory, updating the room contents, calling verbs) happen in a "shopping cart" that only becomes
real when the command finishes successfully. If another player also types `get sword` at the same time, one command
succeeds and the other automatically retries (probably getting "I don't see that here" since the sword is gone).

## Why mooR uses transactions

### The problems with "taking turns"

Without careful coordination, allowing multiple commands to run simultaneously can cause serious concurrency problems:

- **Race conditions**: Two players try to pick up the same sword. Without transactions, both might think they succeeded,
  leading to duplicate objects or corrupted data.
- **Inconsistent states**: Player A drops a sword (updating their inventory) while Player B examines the room (reading
  the room contents). B might see half-completed changes.
- **Deadlocks**: Player A tries to trade with Player B while Player B tries to trade with Player A. Without proper
  coordination, both commands could get stuck waiting for each other forever.

These concurrency problems happen when multiple processes try to change shared data at the same time without proper
coordination. Classic LambdaMOO and ToastStunt avoid these problems by having every single command take turns modifying
the database—only one command can run at a time.

This "taking turns" approach works, but creates a major performance bottleneck: if one command takes too long, everyone
else has to wait. This causes "lag spikes" where the entire MOO freezes until the slow command finishes. It's like
having only one cashier at a busy store: everyone has to wait in line.

> **Historical Context: The LambdaMOO Lag Problem**
>
> Back in the 1990s, when LambdaMOO was a really hot and happening place with hundreds of players connected at a time
> (and running on *much* smaller computers), this lag was a very serious problem. Players would sometimes wait 30
> seconds or more for their commands to execute during busy
> periods. On today's lightly populated MUDs with a handful of players running on very powerful machines, this is barely
> noticed. But mooR's vision is to try to create really popular systems again, so solving this concurrency problem was
> important for us.

### mooR's solution

mooR uses a modern approach to let multiple commands run at the same time safely. It works by giving every command a
perfect, consistent **snapshot** of the world the moment it starts.

Think of it like pausing a movie. Even if other players are moving around or changing things, your command sees the
world exactly as it was when you started typing. You make your changes to this "snapshot," and if nobody else touched
the same things you did, your changes are merged back in instantly.

The database automatically avoids most race conditions and deadlocks. If two commands conflict on a specific piece of
data, the database detects this and automatically retries one of them—no corruption, no deadlocks, and no waiting in
line.

**Trade-offs:**

- mooR can be slower for a single user due to transaction overhead.

- It's much faster when many users are online and active.

- It prevents "Lost Updates" and "Dirty Reads," ensuring you always see a consistent view of the world.

## How transactions keep things consistent

mooR offers a consistent isolation level, which is a fancy way of saying that even though multiple commands might be
running at the same time, the end result is almost always the same as if they happened one after another.

**What this means for you:**

- Changes you make aren't visible to other players until your command finishes.

- You won't see half-completed changes from other players' commands.

- The database stays consistent even with lots of activity.

If two commands try to change the same thing at the same time (like two players trying to pick up the same object), mooR
detects this conflict and automatically retries one of the commands.

### Smart Conflict Resolution

mooR includes a special optimization for **identical changes**. If two players try to set a value to the *exact same
thing* (for example, two workers both trying to ensure a "lights_on" property is set to `1`), mooR recognizes that the
result is identical and allows both to succeed without a conflict. This reduces unnecessary retries in high-concurrency
situations.

## When do transactions happen?

Every command you type starts a new transaction. When you type `look around` or `get sword`, the server:

1. **Starts a transaction** - like opening a shopping cart
2. **Runs your command** - all the verbs and database changes go into the cart
3. **Commits the transaction** - finalizes all changes at once when the command finishes

This means that when a user types a command like `pet the kitty`, the server starts a new transaction right after the
command is parsed, and commits it when the command finishes executing. Any changes the command makes to the database
stay "invisible" to other players until the whole command is done.

## What happens when commands conflict?

Sometimes two players try to do things that conflict—like both trying to pick up the same object at the exact same time.
When this happens:

1. **mooR detects the conflict** when trying to finalize the transactions
2. **One command succeeds** and its changes become real
3. **The other command automatically retries** from the beginning
4. **Usually the retry succeeds** because the situation has changed

You don't have to do anything special—mooR handles this automatically. The retried command might behave differently the
second time (maybe the object is gone now), but that's the correct behavior.

## Output and user interaction

When your command runs, any text it outputs (using functions like `notify()`) gets saved up and only sent to you when
the command finishes successfully. This means:

- If a command gets retried due to conflicts, you won't see duplicate messages
- You only see the output from the final, successful run of the command
- Messages appear all at once when the command completes

This keeps the output clean and prevents confusing partial results from appearing on your screen.

> **Technical Details: Snapshot Isolation (SI)**
>
> For those familiar with database systems, mooR implements **Snapshot Isolation (SI)** with write-write conflict
> detection. This provides strong consistency guarantees with excellent performance for typical MOO workloads.
>
> **What you get:**
> - Every command sees a consistent "snapshot" of the database from the moment it started.
> - **No Lost Updates**: If two commands try to change the same specific piece of data, the conflict is caught.
> - **No Dirty Reads**: You never see half-finished work from others.
>
> **How Conflicts Are Detected:**
> mooR tracks timestamps for each piece of data. During commit, it checks whether any data you're **writing** has
> been modified by another transaction that committed after you started. This catches write-write conflicts on the
> same key.
>
> **The Trade-off: Write Skew**
> Unlike full Serializable Snapshot Isolation (SSI), mooR does not track read sets, which means it's vulnerable to
> **write skew**. This happens when two transactions read overlapping data but write to *different* keys, each
> making a decision that would be invalid if they could see the other's write.
>
> Example: Two doctors checking if they can go off-call by reading both `doctor1_on_call` and `doctor2_on_call`,
> then each writing only their own property. Both see the other is on-call, both go off-call, now no one is on-call.
>
> In practice, this is rarely an issue for MOO. When it matters, you can prevent it by "touching" (writing to) the
> data you depend on to force a conflict check, or by storing related data in a single property.
>
> **Limitations:**
> - **Not Linearizable**: Real-time ordering isn't guaranteed. Transactions might appear to execute in a different
>   order than their actual wall-clock timing.
> - **External Side Effects**: Network calls or other external operations might happen in a different order than
>   the final transaction commit order.
>
> This design prioritizes performance and simplicity over strict serializability, which is well-suited for MOO's
> interactive nature.
