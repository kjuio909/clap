# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_my_app_global_optspecs
    string join \n h/help
end

function __fish_my_app_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_my_app_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_my_app_using_subcommand
    # Check that the subcommands on the command line contain the expected
    # chain of subcommands, in order.
    set -l expected $argv
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_my_app_global_optspecs) -- $cmd 2>/dev/null
    or return
    for token in $argv
        if set -q expected[1]; and test "$token" = "$expected[1]"
            set -e expected[1]
        end
    end
    not set -q expected[1]
end

complete -c my-app -n "__fish_my_app_needs_command" -s h -l help -d 'Print help'
complete -c my-app -n "__fish_my_app_needs_command" -f -a "external" -d 'An external subcommand'
complete -c my-app -n "__fish_my_app_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c my-app -n "__fish_my_app_using_subcommand external" -s h -l help -d 'Print help'
complete -c my-app -n "__fish_my_app_using_subcommand help; and not __fish_my_app_using_subcommand help external; and not __fish_my_app_using_subcommand help help" -f -a "external" -d 'An external subcommand'
complete -c my-app -n "__fish_my_app_using_subcommand help; and not __fish_my_app_using_subcommand help external; and not __fish_my_app_using_subcommand help help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
