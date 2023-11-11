# App Lifecycle

This document will detail the:
+ startup
+ shutdown
+ pause
+ background
+ on-mutate

work that the GUI apps complete.

# Startup

1. Sync

# Shutdown

1. Save document
2. Sync

# Pause

This could be a user switching apps, locking their screen, or on desktop it could be a "loss of focus" event.

1. Save document
2. Sync

# Background

## Auto-save

When lb-rs saves, it compresses, encrypts, and writes the result to a file. Given the unbounded length of files, this can
be expensive. It's also a fundamentally important operation. So we'd like to do it as often as possible, without draining
a user's battery or introducing latency in their typing experience.

Generally auto save should happen after some amount of time elapses while the user has stopped typing. Swift's combine
framework expresses this as the `debounce`. The linux client saves when `last_edit - last_saved > 2 seconds` (on another thread).

## Auto-sync

Syncing is expensive because it's a network operation. It's also a bit tricky because if the same file is being edited
by lots of people simultaneously, then syncing often could cause them to have to resolve conflicts often. While certainly
we could expose this as a preference if people found it useful, initially we're going to take the approach that we want to
sync frequently enough that you never have work lingering on a device that is not accessible to you at the moment. Syncing
on shutdown / pause goes a long way to ensure this, but syncing periodically while the app is open also helps, however we
don't want to sync while the user is actively typing. Ideally we'll sync after the user has been idle for some number of minutes.
And we will sync periodically after that. Reasonable default values are: idle for 1 minute, every 30 minutes after that.

Syncing could change the open file (or even delete it), so care should be taken to properly handle these situations 
[TODO](https://github.com/lockbook/lockbook/issues/558)

# On-mutate

Let's define a mutation to be anything that causes a unit of work to be generated within lb-rs. During such an event, clients
should (depending on their implementation) refresh their file tree, and perform a local-only-calculate-work.