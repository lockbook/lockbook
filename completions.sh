
_lockbook_complete_()
{
    _COMP_OUTPUTSTR="$( lockbook complete -- "${COMP_WORDS[*]}" ${COMP_CWORD} )"
    if test $? -ne 0; then
        return 1
    fi
    COMPREPLY=($( echo -n "$_COMP_OUTPUTSTR" ))
}

complete -o nospace -F _lockbook_complete_ lockbook -E
#compdef lockbook

autoload -U is-at-least

_lockbook() {
    typeset -A opt_args
    typeset -a _arguments_options
    local ret=1

    if is-at-least 5.2; then
        _arguments_options=(-s -S -C)
    else
        _arguments_options=(-s -C)
    fi

    local context curcontext="$curcontext" state line
    _arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
":: :_lockbook_commands" \
"*::: :->lockbook-cli" \
&& ret=0
    case $state in
    (lockbook-cli)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:lockbook-command-$line[1]:"
        case $line[1] in
            (account)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
":: :_lockbook__account_commands" \
"*::: :->account" \
&& ret=0

    case $state in
    (account)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:lockbook-account-command-$line[1]:"
        case $line[1] in
            (new)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
'::username -- your desired username (will prompt if not provided):' \
'::api_url -- the server url to register with (will default first to API_URL then to the lockbook server):' \
&& ret=0
;;
(import)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(export)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(subscribe)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(unsubscribe)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(status)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" \
":: :_lockbook__account__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:lockbook-account-help-command-$line[1]:"
        case $line[1] in
            (new)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(import)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(export)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(subscribe)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(unsubscribe)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(status)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
;;
(debug)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
":: :_lockbook__debug_commands" \
"*::: :->debug" \
&& ret=0

    case $state in
    (debug)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:lockbook-debug-command-$line[1]:"
        case $line[1] in
            (validate)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(info)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
':target -- lockbook file path or ID:' \
&& ret=0
;;
(whoami)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(whereami)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" \
":: :_lockbook__debug__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:lockbook-debug-help-command-$line[1]:"
        case $line[1] in
            (validate)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(info)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(whoami)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(whereami)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
;;
(delete)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
':target -- lockbook file path or ID:' \
&& ret=0
;;
(edit)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
'::target -- lockbook file path or ID:' \
&& ret=0
;;
(export)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
':target -- the path or id of a lockbook folder:' \
'::dest -- a filesystem directory (defaults to current directory):_files' \
&& ret=0
;;
(list)
_arguments "${_arguments_options[@]}" \
'-r[include all children of the given directory, recursively]' \
'--recursive[include all children of the given directory, recursively]' \
'-l[include more info (such as the file ID)]' \
'--long[include more info (such as the file ID)]' \
'--paths[display absolute paths instead of just names]' \
'--dirs[only show directories]' \
'--docs[only show documents]' \
'--ids[print full UUIDs instead of truncated ones]' \
'-h[Print help]' \
'--help[Print help]' \
'::directory -- file path location whose files will be listed:' \
&& ret=0
;;
(move)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
':src_target -- lockbook file path or ID of the file to move:' \
':dest_target -- lockbook file path or ID of the new parent:' \
&& ret=0
;;
(new)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
':path -- lockbook file path:' \
&& ret=0
;;
(print)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
':target -- lockbook file path or ID:' \
&& ret=0
;;
(rename)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
':target -- lockbook file path or ID:' \
':new_name -- the file'\''s new name:' \
&& ret=0
;;
(share)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
":: :_lockbook__share_commands" \
"*::: :->share" \
&& ret=0

    case $state in
    (share)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:lockbook-share-command-$line[1]:"
        case $line[1] in
            (new)
_arguments "${_arguments_options[@]}" \
'--ro[read-only (the other user will not be able to edit the shared file)]' \
'-h[Print help]' \
'--help[Print help]' \
':target -- ID or path of the file you will share:' \
':username -- username of who you would like to share with:' \
&& ret=0
;;
(pending)
_arguments "${_arguments_options[@]}" \
'--full-ids[display full file IDs instead of prefixes]' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(accept)
_arguments "${_arguments_options[@]}" \
'--name=[]:NAME: ' \
'-h[Print help]' \
'--help[Print help]' \
':target -- ID (full or prefix) of a pending share:' \
'::dest -- lockbook file path or ID:' \
&& ret=0
;;
(delete)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
':target -- ID (full or prefix) of a pending share:' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" \
":: :_lockbook__share__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:lockbook-share-help-command-$line[1]:"
        case $line[1] in
            (new)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(pending)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(accept)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(delete)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
;;
(sync)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(completions)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
':shell:(bash elvish fish powershell zsh)' \
&& ret=0
;;
(complete)
_arguments "${_arguments_options[@]}" \
'-h[Print help]' \
'--help[Print help]' \
':input:' \
':current:' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" \
":: :_lockbook__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:lockbook-help-command-$line[1]:"
        case $line[1] in
            (account)
_arguments "${_arguments_options[@]}" \
":: :_lockbook__help__account_commands" \
"*::: :->account" \
&& ret=0

    case $state in
    (account)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:lockbook-help-account-command-$line[1]:"
        case $line[1] in
            (new)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(import)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(export)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(subscribe)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(unsubscribe)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(status)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
        esac
    ;;
esac
;;
(debug)
_arguments "${_arguments_options[@]}" \
":: :_lockbook__help__debug_commands" \
"*::: :->debug" \
&& ret=0

    case $state in
    (debug)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:lockbook-help-debug-command-$line[1]:"
        case $line[1] in
            (validate)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(info)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(whoami)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(whereami)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
        esac
    ;;
esac
;;
(delete)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(edit)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(export)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(list)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(move)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(new)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(print)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(rename)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(share)
_arguments "${_arguments_options[@]}" \
":: :_lockbook__help__share_commands" \
"*::: :->share" \
&& ret=0

    case $state in
    (share)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:lockbook-help-share-command-$line[1]:"
        case $line[1] in
            (new)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(pending)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(accept)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(delete)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
        esac
    ;;
esac
;;
(sync)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(completions)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(complete)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
}

(( $+functions[_lockbook_commands] )) ||
_lockbook_commands() {
    local commands; commands=(
'account:account related commands' \
'debug:import files from your file system into lockbook investigative commands' \
'delete:delete a file' \
'edit:edit a document' \
'export:export a lockbook file to your file system' \
'list:list files and file information' \
'move:move a file to a new parent' \
'new:create a new file at the given path or do nothing if it exists' \
'print:print a document to stdout' \
'rename:rename a file' \
'share:sharing related commands' \
'sync:file sync' \
'completions:generate cli completions' \
'complete:' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'lockbook commands' commands "$@"
}
(( $+functions[_lockbook__help__share__accept_commands] )) ||
_lockbook__help__share__accept_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help share accept commands' commands "$@"
}
(( $+functions[_lockbook__share__accept_commands] )) ||
_lockbook__share__accept_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook share accept commands' commands "$@"
}
(( $+functions[_lockbook__share__help__accept_commands] )) ||
_lockbook__share__help__accept_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook share help accept commands' commands "$@"
}
(( $+functions[_lockbook__account_commands] )) ||
_lockbook__account_commands() {
    local commands; commands=(
'new:create a new lockbook account' \
'import:import an existing account by piping in the account string' \
'export:reveal your account'\''s private key' \
'subscribe:start a monthly subscription for massively increased storage' \
'unsubscribe:cancel an existing subscription' \
'status:show your account status' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'lockbook account commands' commands "$@"
}
(( $+functions[_lockbook__help__account_commands] )) ||
_lockbook__help__account_commands() {
    local commands; commands=(
'new:create a new lockbook account' \
'import:import an existing account by piping in the account string' \
'export:reveal your account'\''s private key' \
'subscribe:start a monthly subscription for massively increased storage' \
'unsubscribe:cancel an existing subscription' \
'status:show your account status' \
    )
    _describe -t commands 'lockbook help account commands' commands "$@"
}
(( $+functions[_lockbook__complete_commands] )) ||
_lockbook__complete_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook complete commands' commands "$@"
}
(( $+functions[_lockbook__help__complete_commands] )) ||
_lockbook__help__complete_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help complete commands' commands "$@"
}
(( $+functions[_lockbook__completions_commands] )) ||
_lockbook__completions_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook completions commands' commands "$@"
}
(( $+functions[_lockbook__help__completions_commands] )) ||
_lockbook__help__completions_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help completions commands' commands "$@"
}
(( $+functions[_lockbook__debug_commands] )) ||
_lockbook__debug_commands() {
    local commands; commands=(
'validate:helps find invalid states within lockbook' \
'info:print metadata associated with a file' \
'whoami:print who is logged into this lockbook' \
'whereami:print information about where this lockbook is stored and its server url' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'lockbook debug commands' commands "$@"
}
(( $+functions[_lockbook__help__debug_commands] )) ||
_lockbook__help__debug_commands() {
    local commands; commands=(
'validate:helps find invalid states within lockbook' \
'info:print metadata associated with a file' \
'whoami:print who is logged into this lockbook' \
'whereami:print information about where this lockbook is stored and its server url' \
    )
    _describe -t commands 'lockbook help debug commands' commands "$@"
}
(( $+functions[_lockbook__delete_commands] )) ||
_lockbook__delete_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook delete commands' commands "$@"
}
(( $+functions[_lockbook__help__delete_commands] )) ||
_lockbook__help__delete_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help delete commands' commands "$@"
}
(( $+functions[_lockbook__help__share__delete_commands] )) ||
_lockbook__help__share__delete_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help share delete commands' commands "$@"
}
(( $+functions[_lockbook__share__delete_commands] )) ||
_lockbook__share__delete_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook share delete commands' commands "$@"
}
(( $+functions[_lockbook__share__help__delete_commands] )) ||
_lockbook__share__help__delete_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook share help delete commands' commands "$@"
}
(( $+functions[_lockbook__edit_commands] )) ||
_lockbook__edit_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook edit commands' commands "$@"
}
(( $+functions[_lockbook__help__edit_commands] )) ||
_lockbook__help__edit_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help edit commands' commands "$@"
}
(( $+functions[_lockbook__account__export_commands] )) ||
_lockbook__account__export_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook account export commands' commands "$@"
}
(( $+functions[_lockbook__account__help__export_commands] )) ||
_lockbook__account__help__export_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook account help export commands' commands "$@"
}
(( $+functions[_lockbook__export_commands] )) ||
_lockbook__export_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook export commands' commands "$@"
}
(( $+functions[_lockbook__help__account__export_commands] )) ||
_lockbook__help__account__export_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help account export commands' commands "$@"
}
(( $+functions[_lockbook__help__export_commands] )) ||
_lockbook__help__export_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help export commands' commands "$@"
}
(( $+functions[_lockbook__account__help_commands] )) ||
_lockbook__account__help_commands() {
    local commands; commands=(
'new:create a new lockbook account' \
'import:import an existing account by piping in the account string' \
'export:reveal your account'\''s private key' \
'subscribe:start a monthly subscription for massively increased storage' \
'unsubscribe:cancel an existing subscription' \
'status:show your account status' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'lockbook account help commands' commands "$@"
}
(( $+functions[_lockbook__account__help__help_commands] )) ||
_lockbook__account__help__help_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook account help help commands' commands "$@"
}
(( $+functions[_lockbook__debug__help_commands] )) ||
_lockbook__debug__help_commands() {
    local commands; commands=(
'validate:helps find invalid states within lockbook' \
'info:print metadata associated with a file' \
'whoami:print who is logged into this lockbook' \
'whereami:print information about where this lockbook is stored and its server url' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'lockbook debug help commands' commands "$@"
}
(( $+functions[_lockbook__debug__help__help_commands] )) ||
_lockbook__debug__help__help_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook debug help help commands' commands "$@"
}
(( $+functions[_lockbook__help_commands] )) ||
_lockbook__help_commands() {
    local commands; commands=(
'account:account related commands' \
'debug:import files from your file system into lockbook investigative commands' \
'delete:delete a file' \
'edit:edit a document' \
'export:export a lockbook file to your file system' \
'list:list files and file information' \
'move:move a file to a new parent' \
'new:create a new file at the given path or do nothing if it exists' \
'print:print a document to stdout' \
'rename:rename a file' \
'share:sharing related commands' \
'sync:file sync' \
'completions:generate cli completions' \
'complete:' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'lockbook help commands' commands "$@"
}
(( $+functions[_lockbook__help__help_commands] )) ||
_lockbook__help__help_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help help commands' commands "$@"
}
(( $+functions[_lockbook__share__help_commands] )) ||
_lockbook__share__help_commands() {
    local commands; commands=(
'new:share a file with another lockbook user' \
'pending:list pending shares' \
'accept:accept a pending by adding it to your file tree' \
'delete:delete a pending share' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'lockbook share help commands' commands "$@"
}
(( $+functions[_lockbook__share__help__help_commands] )) ||
_lockbook__share__help__help_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook share help help commands' commands "$@"
}
(( $+functions[_lockbook__account__help__import_commands] )) ||
_lockbook__account__help__import_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook account help import commands' commands "$@"
}
(( $+functions[_lockbook__account__import_commands] )) ||
_lockbook__account__import_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook account import commands' commands "$@"
}
(( $+functions[_lockbook__help__account__import_commands] )) ||
_lockbook__help__account__import_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help account import commands' commands "$@"
}
(( $+functions[_lockbook__debug__help__info_commands] )) ||
_lockbook__debug__help__info_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook debug help info commands' commands "$@"
}
(( $+functions[_lockbook__debug__info_commands] )) ||
_lockbook__debug__info_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook debug info commands' commands "$@"
}
(( $+functions[_lockbook__help__debug__info_commands] )) ||
_lockbook__help__debug__info_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help debug info commands' commands "$@"
}
(( $+functions[_lockbook__help__list_commands] )) ||
_lockbook__help__list_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help list commands' commands "$@"
}
(( $+functions[_lockbook__list_commands] )) ||
_lockbook__list_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook list commands' commands "$@"
}
(( $+functions[_lockbook__help__move_commands] )) ||
_lockbook__help__move_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help move commands' commands "$@"
}
(( $+functions[_lockbook__move_commands] )) ||
_lockbook__move_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook move commands' commands "$@"
}
(( $+functions[_lockbook__account__help__new_commands] )) ||
_lockbook__account__help__new_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook account help new commands' commands "$@"
}
(( $+functions[_lockbook__account__new_commands] )) ||
_lockbook__account__new_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook account new commands' commands "$@"
}
(( $+functions[_lockbook__help__account__new_commands] )) ||
_lockbook__help__account__new_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help account new commands' commands "$@"
}
(( $+functions[_lockbook__help__new_commands] )) ||
_lockbook__help__new_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help new commands' commands "$@"
}
(( $+functions[_lockbook__help__share__new_commands] )) ||
_lockbook__help__share__new_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help share new commands' commands "$@"
}
(( $+functions[_lockbook__new_commands] )) ||
_lockbook__new_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook new commands' commands "$@"
}
(( $+functions[_lockbook__share__help__new_commands] )) ||
_lockbook__share__help__new_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook share help new commands' commands "$@"
}
(( $+functions[_lockbook__share__new_commands] )) ||
_lockbook__share__new_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook share new commands' commands "$@"
}
(( $+functions[_lockbook__help__share__pending_commands] )) ||
_lockbook__help__share__pending_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help share pending commands' commands "$@"
}
(( $+functions[_lockbook__share__help__pending_commands] )) ||
_lockbook__share__help__pending_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook share help pending commands' commands "$@"
}
(( $+functions[_lockbook__share__pending_commands] )) ||
_lockbook__share__pending_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook share pending commands' commands "$@"
}
(( $+functions[_lockbook__help__print_commands] )) ||
_lockbook__help__print_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help print commands' commands "$@"
}
(( $+functions[_lockbook__print_commands] )) ||
_lockbook__print_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook print commands' commands "$@"
}
(( $+functions[_lockbook__help__rename_commands] )) ||
_lockbook__help__rename_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help rename commands' commands "$@"
}
(( $+functions[_lockbook__rename_commands] )) ||
_lockbook__rename_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook rename commands' commands "$@"
}
(( $+functions[_lockbook__help__share_commands] )) ||
_lockbook__help__share_commands() {
    local commands; commands=(
'new:share a file with another lockbook user' \
'pending:list pending shares' \
'accept:accept a pending by adding it to your file tree' \
'delete:delete a pending share' \
    )
    _describe -t commands 'lockbook help share commands' commands "$@"
}
(( $+functions[_lockbook__share_commands] )) ||
_lockbook__share_commands() {
    local commands; commands=(
'new:share a file with another lockbook user' \
'pending:list pending shares' \
'accept:accept a pending by adding it to your file tree' \
'delete:delete a pending share' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'lockbook share commands' commands "$@"
}
(( $+functions[_lockbook__account__help__status_commands] )) ||
_lockbook__account__help__status_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook account help status commands' commands "$@"
}
(( $+functions[_lockbook__account__status_commands] )) ||
_lockbook__account__status_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook account status commands' commands "$@"
}
(( $+functions[_lockbook__help__account__status_commands] )) ||
_lockbook__help__account__status_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help account status commands' commands "$@"
}
(( $+functions[_lockbook__account__help__subscribe_commands] )) ||
_lockbook__account__help__subscribe_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook account help subscribe commands' commands "$@"
}
(( $+functions[_lockbook__account__subscribe_commands] )) ||
_lockbook__account__subscribe_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook account subscribe commands' commands "$@"
}
(( $+functions[_lockbook__help__account__subscribe_commands] )) ||
_lockbook__help__account__subscribe_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help account subscribe commands' commands "$@"
}
(( $+functions[_lockbook__help__sync_commands] )) ||
_lockbook__help__sync_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help sync commands' commands "$@"
}
(( $+functions[_lockbook__sync_commands] )) ||
_lockbook__sync_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook sync commands' commands "$@"
}
(( $+functions[_lockbook__account__help__unsubscribe_commands] )) ||
_lockbook__account__help__unsubscribe_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook account help unsubscribe commands' commands "$@"
}
(( $+functions[_lockbook__account__unsubscribe_commands] )) ||
_lockbook__account__unsubscribe_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook account unsubscribe commands' commands "$@"
}
(( $+functions[_lockbook__help__account__unsubscribe_commands] )) ||
_lockbook__help__account__unsubscribe_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help account unsubscribe commands' commands "$@"
}
(( $+functions[_lockbook__debug__help__validate_commands] )) ||
_lockbook__debug__help__validate_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook debug help validate commands' commands "$@"
}
(( $+functions[_lockbook__debug__validate_commands] )) ||
_lockbook__debug__validate_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook debug validate commands' commands "$@"
}
(( $+functions[_lockbook__help__debug__validate_commands] )) ||
_lockbook__help__debug__validate_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help debug validate commands' commands "$@"
}
(( $+functions[_lockbook__debug__help__whereami_commands] )) ||
_lockbook__debug__help__whereami_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook debug help whereami commands' commands "$@"
}
(( $+functions[_lockbook__debug__whereami_commands] )) ||
_lockbook__debug__whereami_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook debug whereami commands' commands "$@"
}
(( $+functions[_lockbook__help__debug__whereami_commands] )) ||
_lockbook__help__debug__whereami_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help debug whereami commands' commands "$@"
}
(( $+functions[_lockbook__debug__help__whoami_commands] )) ||
_lockbook__debug__help__whoami_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook debug help whoami commands' commands "$@"
}
(( $+functions[_lockbook__debug__whoami_commands] )) ||
_lockbook__debug__whoami_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook debug whoami commands' commands "$@"
}
(( $+functions[_lockbook__help__debug__whoami_commands] )) ||
_lockbook__help__debug__whoami_commands() {
    local commands; commands=()
    _describe -t commands 'lockbook help debug whoami commands' commands "$@"
}

if [ "$funcstack[1]" = "_lockbook" ]; then
    _lockbook "$@"
else
    compdef _lockbook lockbook
fi
