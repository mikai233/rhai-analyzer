fn analyze(files, settings) {
  let mapper = |file| #{
    name: file.name,
    changed: file?.meta?[0],
    labels: [`lint:${file.name}`, `owner:${settings.owner}`]
  };

  let summary = switch files.len() {
    0 => `empty`,
    1 | 2 => `small`,
    _ => {
      let total = files.len() + settings.extra;
      `many:${total}`
    }
  };

  process(mapper(files[0]), summary, settings.mode);
}
