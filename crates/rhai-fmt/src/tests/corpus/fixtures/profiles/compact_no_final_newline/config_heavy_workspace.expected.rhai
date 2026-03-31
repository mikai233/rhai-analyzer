import "env" as env;
import "workspace" as workspace;

export const DEFAULTS = #{
  name: "demo",
  watch: true,
  paths: ["src", "tests", "scripts"],
  limits: #{warn: 80, error: 120},
  tasks: ["fmt", "lint", "test"]
};

fn make_config(root, mode) {
  let cache = env::cache_dir();
  let workspace_name = workspace::name(root);
  let config = #{
    root: root,
    mode: mode,
    cache_dir: cache,
    workspace: workspace_name,
    defaults: DEFAULTS,
    hooks: [`prepare:${root}`, `validate:${workspace_name}`, `run:${mode}`]
  };
  config
}
