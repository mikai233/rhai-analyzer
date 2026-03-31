import "env" as env;
import "project" as project;

fn bootstrap(root, flags) {
    let config = #{
        root: root,
        cache: env::cache_dir(),
        strict: flags?.strict,
    };
    let hooks = [`prepare:${root}`, `load:${project::name(root)}`, `watch:${flags.mode}`];
    switch flags.mode {
        `ci` => project::run_ci(config, hooks),
        `dev` => {
            project::run_watch(config, hooks)
        },
        _ => project::run_once(config, hooks)
    }
}
