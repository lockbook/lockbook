complete -c lbdev -f --condition "not __fish_seen_subcommand_from file-command non-file-command" -a '(lbdev complete fish 0 (commandline -cp))'
