# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_my_app_global_optspecs
    string join \n c/config h/help V/version
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

complete -c my-app -n "__fish_my_app_needs_command" -s c -s C -l config -l conf -d 'some config file'
complete -c my-app -n "__fish_my_app_needs_command" -s h -l help -d 'Print help'
complete -c my-app -n "__fish_my_app_needs_command" -s V -l version -d 'Print version'
complete -c my-app -n "__fish_my_app_needs_command" -a "test" -d 'tests things'
complete -c my-app -n "__fish_my_app_needs_command" -a "some_cmd" -d 'top level subcommand'
complete -c my-app -n "__fish_my_app_needs_command" -a "some_cmd_alias" -d 'top level subcommand'
complete -c my-app -n "__fish_my_app_needs_command" -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c my-app -n "__fish_my_app_using_subcommand test" -l case -d 'the case to test' -r
complete -c my-app -n "__fish_my_app_using_subcommand test" -s h -l help -d 'Print help'
complete -c my-app -n "__fish_my_app_using_subcommand test" -s V -l version -d 'Print version'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd; and not __fish_my_app_using_subcommand some_cmd sub_cmd; and not __fish_my_app_using_subcommand some_cmd help" -s h -l help -d 'Print help'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd; and not __fish_my_app_using_subcommand some_cmd sub_cmd; and not __fish_my_app_using_subcommand some_cmd help" -s V -l version -d 'Print version'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd; and not __fish_my_app_using_subcommand some_cmd sub_cmd; and not __fish_my_app_using_subcommand some_cmd help" -f -a "sub_cmd" -d 'sub-subcommand'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd; and not __fish_my_app_using_subcommand some_cmd sub_cmd; and not __fish_my_app_using_subcommand some_cmd help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd sub_cmd" -l config -d 'the other case to test' -r -f -a "Lest quotes\, aren\'t escaped.\t'help,with,comma'
Second to trigger display of options\t''"
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd sub_cmd" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd sub_cmd" -s V -l version -d 'Print version'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd help; and not __fish_my_app_using_subcommand some_cmd help sub_cmd; and not __fish_my_app_using_subcommand some_cmd help help" -f -a "sub_cmd" -d 'sub-subcommand'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd help; and not __fish_my_app_using_subcommand some_cmd help sub_cmd; and not __fish_my_app_using_subcommand some_cmd help help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd_alias; and not __fish_my_app_using_subcommand some_cmd_alias sub_cmd; and not __fish_my_app_using_subcommand some_cmd_alias help" -s h -l help -d 'Print help'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd_alias; and not __fish_my_app_using_subcommand some_cmd_alias sub_cmd; and not __fish_my_app_using_subcommand some_cmd_alias help" -s V -l version -d 'Print version'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd_alias; and not __fish_my_app_using_subcommand some_cmd_alias sub_cmd; and not __fish_my_app_using_subcommand some_cmd_alias help" -f -a "sub_cmd" -d 'sub-subcommand'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd_alias; and not __fish_my_app_using_subcommand some_cmd_alias sub_cmd; and not __fish_my_app_using_subcommand some_cmd_alias help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd_alias sub_cmd" -l config -d 'the other case to test' -r -f -a "Lest quotes\, aren\'t escaped.\t'help,with,comma'
Second to trigger display of options\t''"
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd_alias sub_cmd" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd_alias sub_cmd" -s V -l version -d 'Print version'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd_alias help; and not __fish_my_app_using_subcommand some_cmd_alias help sub_cmd; and not __fish_my_app_using_subcommand some_cmd_alias help help" -f -a "sub_cmd" -d 'sub-subcommand'
complete -c my-app -n "__fish_my_app_using_subcommand some_cmd_alias help; and not __fish_my_app_using_subcommand some_cmd_alias help sub_cmd; and not __fish_my_app_using_subcommand some_cmd_alias help help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c my-app -n "__fish_my_app_using_subcommand help; and not __fish_my_app_using_subcommand help test; and not __fish_my_app_using_subcommand help some_cmd; and not __fish_my_app_using_subcommand help help" -f -a "test" -d 'tests things'
complete -c my-app -n "__fish_my_app_using_subcommand help; and not __fish_my_app_using_subcommand help test; and not __fish_my_app_using_subcommand help some_cmd; and not __fish_my_app_using_subcommand help help" -f -a "some_cmd" -d 'top level subcommand'
complete -c my-app -n "__fish_my_app_using_subcommand help; and not __fish_my_app_using_subcommand help test; and not __fish_my_app_using_subcommand help some_cmd; and not __fish_my_app_using_subcommand help help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c my-app -n "__fish_my_app_using_subcommand help some_cmd; and not __fish_my_app_using_subcommand help some_cmd sub_cmd" -f -a "sub_cmd" -d 'sub-subcommand'
