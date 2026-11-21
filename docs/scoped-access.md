# Scoped Access
Premium users can create subaccounts which can be used to securely give scripts and agents programatic access to your account.

Subaccounts take the form `username+subaccount-name`. They start off with no files of their own. You can share any Document or Folder with a subaccount in read or write mode. They can do the same with any other Lockbook user.

Usage incurred by a subaccount is deducted from the parent account's usage, unless they are editing a file owned by someone else (as usual).

A subaccount's key is derived from your private key (and treated similarly securely). So you don't have to manage multiple keys.

Once a subaccount is deleted, another cannot take it's name.

## Uses for subaccounts:
* [Use github actions to automatically publish a blog from a folder within Lockbook](https://github.com/Parth/parth.cafe/tree/master/.github/workflows)
* Create an app that stores it's data within Lockbook
* Give an agent access to a portion of your Lockbook
* Sign into a portion of your Lockbook in an untrusted environment (work laptop)