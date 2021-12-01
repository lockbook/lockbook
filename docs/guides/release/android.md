# Android Releasing

- Set up your [build environment](/docs/guides/build/android.md:3).

## Github Releases

- Install [github-release](https://github.com/github-release/github-release).
- [Generate a personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token)
and set it to the environment variable `GITHUB_TOKEN`.
- Ensure you have the android release key and password and set it equal to `ANDROID_RELEASE_KEY` and `ANDROID_RELEASE_KEY_PASSWORD` environment variables respectively.
- Run [release_android.sh](/utils/dev/release_android.sh).