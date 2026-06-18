# Sync sigma_beam submodule

When committing changes, always include the `sigma_beam` submodule pointer if it has changed. Run `git add sigma_beam` alongside other staged files so the submodule reference stays in sync with main.

Before pushing, check `git status` for the submodule — if it shows `modified: sigma_beam (new commits)`, stage and commit it (or include it in the current commit).
