# Design Constraints

We want to release often. Why?
 
1. We want to reward people for reporting bugs to us. The best way to do this is to get the bugfix in their hands as quickly as possible.
1. We want to perform intentional QA and balance the QA workload. Too little, and the engineering interruption feels like a waste. Too much and QA quality will suffer.
1. Our release infrastructure is non-trivial and fequently exersizing it let's us catch issues early and be selective about when we fix problems.

However, in most cases: users cannot rollback. So we want to be confident in the increments we're shipping. The following schedule and roles are crafted to balance QA workload and risk of changes.

# Release Commander
**Release Commander** will be Adam, Parth, or Travis. They will be chosen collectively based on the goals of the release. Some consideration includes:
* who is shipping the most
* or who is shipping the riskiest thing (migration, several refinements to an experience)
* or who is shipping the most important thing.

The release Commander will have the following responsibilities:
* Ensure we stick to the schedule in the next section 
* Delegate out QA to whatever maintainers & volunteers we have present
* Operate the release machinery
* Communicate the released changes to Github & Discord

# The 7 day cycle
1. After a successful release (Thursday Afternoon)
   1. Release Commander, enhances the automatically generated github release with any visually stimulating content. Ideally in a way that can be easily brought over to discord for minimal effort. For productivity and consistency feel free to say *We did X* even if you weren't directly involved. The release Commander is representing the whole Lockbook team. Feel free to link the PR if this feels wrong, this is an open source project and the whole audit trail is aparent.
   1. Release Commander chooses a successor.
   1. Merge window opens for contributors (Thursday -> Tuesday). Some guidelines for what to merge when:
      1. Risky things soonest, for maximum dogfooding. Do not merge a risky thing moments before the merge window closes changing the nature of the release. If your change is risky, use all available assets including inviting people to QA your changes directly on your PR. Your obligation ends at the invitation whether they show up to QA the changes or not is not your problem. **Merging your change is an expression that you've taken all the steps you could to ensure the releasibility of your change.**
      1. If your changes are graphical in nature ensure there is visually stimulating content that a Release Commander can use to craft a release message. Gifs are preferred, then videos, then screenshots.
      1.  Communicate what you're merging to `#development`. There are non-technical (non-github) people in our discord who have shown a willingness to test unreleased work. You can link them the automatically produced binary artifacts, or invite them to be Play Store Internal Testers, or TestFlight users. Speak out and try to aquire the QA resources you need.
1. Merge window Closes **Tuesday 7pm**:
    1.  Any dogfooding announcements and requests are made.
    1.  Any merge during this point is at the discretion of the release Commander. Make a PR and tag them if you think this is a low risk bugfix and based on the context of the release they'll determine whether or not you have to wait until post release.
    1.  Release Commander has until **Wednesday 11am** to send out the QA Plan.
1. **Wednesday 11am** QA plan deadline
    1.  Release Commander writes the QA plan in `lb-maintainers/common/release-ops`
    1.  Prefix it with 'pending-' so that 'pending-release-' brings up the correct doc for all team members at any time
    1. Create a thread in `#release-ops`, optimisitically choosing Thursday for the version number of the release (see 'Version numbers' below).
1. **Thursday 11am** QA Results deadline. 3 possible outcomes.
    1.  No bugs found & release can proceed as planned (see 'How to release' below).
    1.  Bugs were found. They are minor, or the work can be cleanly reverted. Possibly another 24 hours of QA is performed and the release happens tomorrow
    1.  Bugs were found and the release is cancelled. Responsible party has until Friday to restore releasibility.

# Version numbers
* We encode a date in our version numbers. Why?
  * Easy to order
  * It's an option everywhere
* Almost all places require these to never decrease. Implicitly most places don't support the idea of rolling back.
* TestFlight
  * On test flight we can have an external group of testers who can access our app just by using a link.
  * Each time we bump the version number, our app needs a fresh review by apple before this group of people can access the app.
* For the above reason, we bump the version number right before we release (so that production version numbers are the most accurate).
* For the app store and testflight. Once we release, that version is "closed" and new builds cannot be submitted. So we additionally bump the version right after we release. This has the added benefit to distinguish dev traffic from actual customer traffic.

# What is risk?
For the purposes of a release a change is "Risky" if it is likely to invalidate QA (requiring a re-request of QA flows), or if it is likely to result in an aborted release. These outcomes prevent known, good value from being shipped.
  * Risk can be reduced by making sure your work can be reverted: clean chunks of work that don't build on one another and can be reverted. And if reverted would not invalidate QA.
  * The change doesn't require feedback from users (bugfix, obvious improvement, etc).
  * The change is isolated to a given platform.

# How to release
1. Github Actions :arrow_right: Bump versions :arrow_right: Today. This will set the version to today's date.
1. Github Actions :arrow_right: Github release. This will spawn the workflow which will create a github release. The inital workflow will generate the github release, tag, and changelog. This workflow will also spawn the automatiions that publish to all the various app stores. Keep a tab open monitoring the progress of these.
1. The server is not released automatically. If a server release is required. At this point, likely only Parth should be doing this. If this becomes annoying we should automate it so more people can do it: 
   1. ssh into prod
   1. `cd lockbook`. 
   1. `git pull`. Check `git log` and ensure the output is what you expect. 
   1. `cd server && cargo build -r`.
   1. if needed take a snapshot of all users data, and the currently running binary. Bring it locally for maximum conservatism. There are automated server backups as well managed by google.
   1. If needed `systemctl stop lockbook-server.service`
   1. `cd ../target/release`. `mv lockbook-server /usr/local/bin`
   1. `systemctl restart lockbook-server.service`
   1. the server tests pagerduty at startup. Expect a page. **Resolve** the page so that the test will work any time the server is restarted.
1. Once the github release exists populate it with any additional context. Make it obvious using markdown features if there are any breaking changes to `lockbook` CLI. 
1. Github Actions :arrow_right: Bump versions :arrow_right: patch. This increments the patch by 1. This will be the dev version number.
1. Appstore connect :arrow_right: promote the test flight build we just released (it will have today's date) to production for iOS and macOS.
1. Flatpak & Nixpkgs will send Parth a pull request make sure he approves it.
1. Announce that the release window is open in `#development`
1. Propegate any information about this release to `#general`. This is a justified use of `@here`. Make it obvious using markdown features if there are any breaking changes to `lockbook` CLI.