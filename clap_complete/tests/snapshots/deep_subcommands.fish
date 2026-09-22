# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_demo_global_optspecs
    string join \n h/help
end

function __fish_demo_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_demo_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_demo_using_subcommand
    set -l cmd (__fish_demo_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

function __fish_demo_using_command_alpha_beta_gamma
    # Match the full ordered subcommand chain leading here, ignoring options.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    set -q cmd[1]
    or return 1
    contains -- $cmd[-1] -- --mode
    and set -a cmd __fish_clap_complete_value
    argparse -i h/help mode= -- $cmd 2>/dev/null
    or return 1
    test (count $argv) -ge 3
    or return 1
    test "$argv[1]" = 'alpha'
    or return 1
    test "$argv[2]" = 'beta'
    or return 1
    test "$argv[3]" = 'gamma'
    or return 1
    contains -- delta $argv[4..]
    and return 1
    contains -- help $argv[4..]
    and return 1
    return 0
end

function __fish_demo_using_command_alpha_beta_gamma_delta
    # Match the full ordered subcommand chain leading here, ignoring options.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    set -q cmd[1]
    or return 1
    contains -- $cmd[-1] -- --mode
    and set -a cmd __fish_clap_complete_value
    argparse -i h/help leaf mode= -- $cmd 2>/dev/null
    or return 1
    test (count $argv) -ge 4
    or return 1
    test "$argv[1]" = 'alpha'
    or return 1
    test "$argv[2]" = 'beta'
    or return 1
    test "$argv[3]" = 'gamma'
    or return 1
    test "$argv[4]" = 'delta'
    or return 1
    return 0
end

function __fish_demo_using_command_alpha_beta_gamma_help
    # Match the full ordered subcommand chain leading here, ignoring options.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    set -q cmd[1]
    or return 1
    contains -- $cmd[-1] -- --mode
    and set -a cmd __fish_clap_complete_value
    argparse -i h/help mode= -- $cmd 2>/dev/null
    or return 1
    test (count $argv) -ge 4
    or return 1
    test "$argv[1]" = 'alpha'
    or return 1
    test "$argv[2]" = 'beta'
    or return 1
    test "$argv[3]" = 'gamma'
    or return 1
    test "$argv[4]" = 'help'
    or return 1
    contains -- delta $argv[5..]
    and return 1
    contains -- help $argv[5..]
    and return 1
    return 0
end

function __fish_demo_using_command_alpha_beta_help
    # Match the full ordered subcommand chain leading here, ignoring options.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    set -q cmd[1]
    or return 1
    argparse -i h/help -- $cmd 2>/dev/null
    or return 1
    test (count $argv) -ge 3
    or return 1
    test "$argv[1]" = 'alpha'
    or return 1
    test "$argv[2]" = 'beta'
    or return 1
    test "$argv[3]" = 'help'
    or return 1
    contains -- gamma $argv[4..]
    and return 1
    contains -- help $argv[4..]
    and return 1
    return 0
end

function __fish_demo_using_command_alpha_beta_help_gamma
    # Match the full ordered subcommand chain leading here, ignoring options.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    set -q cmd[1]
    or return 1
    argparse -i h/help -- $cmd 2>/dev/null
    or return 1
    test (count $argv) -ge 4
    or return 1
    test "$argv[1]" = 'alpha'
    or return 1
    test "$argv[2]" = 'beta'
    or return 1
    test "$argv[3]" = 'help'
    or return 1
    test "$argv[4]" = 'gamma'
    or return 1
    contains -- delta $argv[5..]
    and return 1
    return 0
end

function __fish_demo_using_command_alpha_help_beta
    # Match the full ordered subcommand chain leading here, ignoring options.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    set -q cmd[1]
    or return 1
    argparse -i h/help -- $cmd 2>/dev/null
    or return 1
    test (count $argv) -ge 3
    or return 1
    test "$argv[1]" = 'alpha'
    or return 1
    test "$argv[2]" = 'help'
    or return 1
    test "$argv[3]" = 'beta'
    or return 1
    contains -- gamma $argv[4..]
    and return 1
    return 0
end

function __fish_demo_using_command_alpha_help_beta_gamma
    # Match the full ordered subcommand chain leading here, ignoring options.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    set -q cmd[1]
    or return 1
    argparse -i h/help -- $cmd 2>/dev/null
    or return 1
    test (count $argv) -ge 4
    or return 1
    test "$argv[1]" = 'alpha'
    or return 1
    test "$argv[2]" = 'help'
    or return 1
    test "$argv[3]" = 'beta'
    or return 1
    test "$argv[4]" = 'gamma'
    or return 1
    contains -- delta $argv[5..]
    and return 1
    return 0
end

function __fish_demo_using_command_omega_beta_gamma
    # Match the full ordered subcommand chain leading here, ignoring options.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    set -q cmd[1]
    or return 1
    argparse -i h/help other -- $cmd 2>/dev/null
    or return 1
    test (count $argv) -ge 3
    or return 1
    test "$argv[1]" = 'omega'
    or return 1
    test "$argv[2]" = 'beta'
    or return 1
    test "$argv[3]" = 'gamma'
    or return 1
    return 0
end

function __fish_demo_using_command_omega_beta_help
    # Match the full ordered subcommand chain leading here, ignoring options.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    set -q cmd[1]
    or return 1
    argparse -i h/help -- $cmd 2>/dev/null
    or return 1
    test (count $argv) -ge 3
    or return 1
    test "$argv[1]" = 'omega'
    or return 1
    test "$argv[2]" = 'beta'
    or return 1
    test "$argv[3]" = 'help'
    or return 1
    contains -- gamma $argv[4..]
    and return 1
    contains -- help $argv[4..]
    and return 1
    return 0
end

function __fish_demo_using_command_omega_help_beta
    # Match the full ordered subcommand chain leading here, ignoring options.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    set -q cmd[1]
    or return 1
    argparse -i h/help -- $cmd 2>/dev/null
    or return 1
    test (count $argv) -ge 3
    or return 1
    test "$argv[1]" = 'omega'
    or return 1
    test "$argv[2]" = 'help'
    or return 1
    test "$argv[3]" = 'beta'
    or return 1
    contains -- gamma $argv[4..]
    and return 1
    return 0
end

function __fish_demo_using_command_help_alpha_beta
    # Match the full ordered subcommand chain leading here, ignoring options.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    set -q cmd[1]
    or return 1
    argparse -i h/help -- $cmd 2>/dev/null
    or return 1
    test (count $argv) -ge 3
    or return 1
    test "$argv[1]" = 'help'
    or return 1
    test "$argv[2]" = 'alpha'
    or return 1
    test "$argv[3]" = 'beta'
    or return 1
    contains -- gamma $argv[4..]
    and return 1
    return 0
end

function __fish_demo_using_command_help_alpha_beta_gamma
    # Match the full ordered subcommand chain leading here, ignoring options.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    set -q cmd[1]
    or return 1
    argparse -i h/help -- $cmd 2>/dev/null
    or return 1
    test (count $argv) -ge 4
    or return 1
    test "$argv[1]" = 'help'
    or return 1
    test "$argv[2]" = 'alpha'
    or return 1
    test "$argv[3]" = 'beta'
    or return 1
    test "$argv[4]" = 'gamma'
    or return 1
    contains -- delta $argv[5..]
    and return 1
    return 0
end

function __fish_demo_using_command_help_omega_beta
    # Match the full ordered subcommand chain leading here, ignoring options.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    set -q cmd[1]
    or return 1
    argparse -i h/help -- $cmd 2>/dev/null
    or return 1
    test (count $argv) -ge 3
    or return 1
    test "$argv[1]" = 'help'
    or return 1
    test "$argv[2]" = 'omega'
    or return 1
    test "$argv[3]" = 'beta'
    or return 1
    contains -- gamma $argv[4..]
    and return 1
    return 0
end

complete -c demo -n "__fish_demo_needs_command" -s h -l help -d 'Print help'
complete -c demo -n "__fish_demo_needs_command" -f -a "alpha"
complete -c demo -n "__fish_demo_needs_command" -f -a "omega"
complete -c demo -n "__fish_demo_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c demo -n "__fish_demo_using_subcommand alpha; and not __fish_seen_subcommand_from beta help" -s h -l help -d 'Print help'
complete -c demo -n "__fish_demo_using_subcommand alpha; and not __fish_seen_subcommand_from beta help" -f -a "beta"
complete -c demo -n "__fish_demo_using_subcommand alpha; and not __fish_seen_subcommand_from beta help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c demo -n "__fish_demo_using_subcommand alpha; and __fish_seen_subcommand_from beta" -s h -l help -d 'Print help'
complete -c demo -n "__fish_demo_using_subcommand alpha; and __fish_seen_subcommand_from beta" -f -a "gamma"
complete -c demo -n "__fish_demo_using_subcommand alpha; and __fish_seen_subcommand_from beta" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c demo -n "__fish_demo_using_command_alpha_beta_gamma" -l mode -r -f -a "fast\t''
safe\t''"
complete -c demo -n "__fish_demo_using_command_alpha_beta_gamma" -s h -l help -d 'Print help'
complete -c demo -n "__fish_demo_using_command_alpha_beta_gamma" -f -a "delta"
complete -c demo -n "__fish_demo_using_command_alpha_beta_gamma" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c demo -n "__fish_demo_using_command_alpha_beta_gamma_delta" -l leaf
complete -c demo -n "__fish_demo_using_command_alpha_beta_gamma_delta" -s h -l help -d 'Print help'
complete -c demo -n "__fish_demo_using_command_alpha_beta_gamma_help" -f -a "delta"
complete -c demo -n "__fish_demo_using_command_alpha_beta_gamma_help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c demo -n "__fish_demo_using_command_alpha_beta_help" -f -a "gamma"
complete -c demo -n "__fish_demo_using_command_alpha_beta_help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c demo -n "__fish_demo_using_command_alpha_beta_help_gamma" -f -a "delta"
complete -c demo -n "__fish_demo_using_subcommand alpha; and __fish_seen_subcommand_from help" -f -a "beta"
complete -c demo -n "__fish_demo_using_subcommand alpha; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c demo -n "__fish_demo_using_command_alpha_help_beta" -f -a "gamma"
complete -c demo -n "__fish_demo_using_command_alpha_help_beta_gamma" -f -a "delta"
complete -c demo -n "__fish_demo_using_subcommand omega; and not __fish_seen_subcommand_from beta help" -s h -l help -d 'Print help'
complete -c demo -n "__fish_demo_using_subcommand omega; and not __fish_seen_subcommand_from beta help" -f -a "beta"
complete -c demo -n "__fish_demo_using_subcommand omega; and not __fish_seen_subcommand_from beta help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c demo -n "__fish_demo_using_subcommand omega; and __fish_seen_subcommand_from beta" -s h -l help -d 'Print help'
complete -c demo -n "__fish_demo_using_subcommand omega; and __fish_seen_subcommand_from beta" -f -a "gamma"
complete -c demo -n "__fish_demo_using_subcommand omega; and __fish_seen_subcommand_from beta" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c demo -n "__fish_demo_using_command_omega_beta_gamma" -l other
complete -c demo -n "__fish_demo_using_command_omega_beta_gamma" -s h -l help -d 'Print help'
complete -c demo -n "__fish_demo_using_command_omega_beta_help" -f -a "gamma"
complete -c demo -n "__fish_demo_using_command_omega_beta_help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c demo -n "__fish_demo_using_subcommand omega; and __fish_seen_subcommand_from help" -f -a "beta"
complete -c demo -n "__fish_demo_using_subcommand omega; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c demo -n "__fish_demo_using_command_omega_help_beta" -f -a "gamma"
complete -c demo -n "__fish_demo_using_subcommand help; and not __fish_seen_subcommand_from alpha omega help" -f -a "alpha"
complete -c demo -n "__fish_demo_using_subcommand help; and not __fish_seen_subcommand_from alpha omega help" -f -a "omega"
complete -c demo -n "__fish_demo_using_subcommand help; and not __fish_seen_subcommand_from alpha omega help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c demo -n "__fish_demo_using_subcommand help; and __fish_seen_subcommand_from alpha" -f -a "beta"
complete -c demo -n "__fish_demo_using_command_help_alpha_beta" -f -a "gamma"
complete -c demo -n "__fish_demo_using_command_help_alpha_beta_gamma" -f -a "delta"
complete -c demo -n "__fish_demo_using_subcommand help; and __fish_seen_subcommand_from omega" -f -a "beta"
complete -c demo -n "__fish_demo_using_command_help_omega_beta" -f -a "gamma"
