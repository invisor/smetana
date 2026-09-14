/* Which of two offers a project without `.smetana/project.toml` gets. One
   question in two halves, so the panel's marks, the tile's menu and the
   dialog opened after "Add project" cannot come to disagree: `needsSetup` is
   "no file and something in the folder to describe", `needsStart` is "no file
   and nothing but housekeeping". `empty` is Rust's answer (`survey::is_empty`),
   carried on the `project_config` reply, and a reply without it reads as a
   populated folder — the setup agent knows how to ask, and the founding agent
   would promise a foundation in a folder nobody has looked at.

   Pure, no Vue and no DOM in it, of the `projectDefaults.js` family beside it:
   a `.vue` file is the one thing no test in this repository can reach, so the
   whole of this rule lives outside every component that draws from it. */
export function needsSetup(config) {
  return config?.state === 'missing' && config.empty !== true
}

export function needsStart(config) {
  return config?.state === 'missing' && config.empty === true
}
