* We want to release often
  * We want to reward people for reporting bugs to us. The best way to do this is to get the bugfix in their hands as quickly as possible.
  * Our release infrastructure is non-trivial and fequently exersizing it let's us catch issues early and be selective about when we fix problems.
  * Users cannot rollback, so we want to be confident in the increments we're shipping. The following schedule is crafted to balance QA workload and risk of changes
* Release Schedule
  * Monday 11am EST -- release coordination
    * **Release Coordinator** is selected based on who is shipping the most this release. They're going to gather any QA resources (internal or external) fan out work and evaluate release suitability. QA people have 24 hours to perform QA. 11am is the deadline for sending the message to testers.
  * Tuesday 11am EST -- Decision time
    * Plan A: no bugs found and release can proceed as planned. 
    * Plan B: bugs were found and the release is cancelled. Responsible party has until Friday to reconcile the situation. 
    * Plan C: work can be cleanly reverted, or minor bugfixes exist. Another 24 hours of QA is scheduled, and **Decision time** is repeated on Wednesday.
  * Tuesday Afternoon -- Post Release
    * **Release Coordinator** goes through and crafts a message containing gifs / videos and updates the github release. This message should be compatible with discord, and general is notified. Do not wait for app store acceptance or any other external events. If a github release exists, make discord and github satisfying spaces to consume what happened. 
    * **Merge Window Opens**:
      * If your change is graphical in nature, at a minimum your PR should contain a screenshot. A gif is preferred, and a video is acceptable. This makes it easy for a release coordinator to make a post on your behalf.
      * Merging risky things: Take all the steps you can to de-risk your merge. Before the merge window opened you should have invited people to try your PR (if relevant). Leave an audit trail in your PR of what QA was done. Your obligation ends at the invitation, whether the person shows up or not is not your problem. If you want a larger window of testing and don't want to release the following monday, set expectations accordingly.
      * After the merge window opens, merge riskiest things first, communicate what you're merging to `#development`, what the nature of the risk is and who should be dogfooding it. Ask people to rebase their branches so they can passively experience the changes while they work on their own stuff.
  * Friday 7pm **Merge Window Closes** / **Passive Dogfood Begins**:
    * External testers notified, they can give any feedback during the weekend at their leisure 
    * Any merging during this point should be overwhelmbly obviously low in risk, or a specific bugfix in service of the release.
    * A release coordinator is determined for Monday.

# Stacked PRs
If you're concerned about juggling the git history while merging is not an option ChatGPT got you:

You can open a PR that depends on another unmerged PR.

Assume:

```text
master
  \
   A   PR 1
    \
     B   PR 2
```

There are three practical workflows.

### 1. Squash before branching

Make PR 1 a single commit before creating PR 2:

```text
master
  \
   A   PR 1
    \
     B   PR 2
```

When PR 1 is squash-merged, commit `A` lands unchanged. Git then understands that PR 2 contains only `B`.

### 2. Rebase only the dependent commits

Let PR 1 contain multiple commits:

```text
master
  \
   A---B   PR 1
        \
         C---D   PR 2
```

After PR 1 is squash-merged, replay only `C` and `D`:

```bash
git rebase --onto master PR_1_BRANCH PR_2_BRANCH
```

Result:

```text
master---S
          \
           C'---D'   PR 2
```

`S` is the squashed version of `A---B`.

### 3. Ignore commit structure and merge `master`

Create PR 2 from `master`, then bring PR 1’s changes into it:

```bash
git switch -c pr-2 master
git reset --soft PR_1_BRANCH
git commit
```

The branches remain independent:

```text
master
  ├── A     PR 1
  └── A+B   PR 2
```

After PR 1 is merged:

```bash
git switch pr-2
git pull --no-rebase origin master
```

The merge removes PR 1’s changes from PR 2’s diff, leaving only the additional work.

This creates messier branch history, but squash-merging keeps `master` clean.
