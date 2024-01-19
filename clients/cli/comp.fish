complete -c lockbook -f --condition "not __fish_seen_subcommand_from file-command non-file-command" -a '(lockbook complete fish 0 (commandline -cp))'
