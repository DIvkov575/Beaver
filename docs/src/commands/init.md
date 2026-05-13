# init

Scaffold a new `beaver_config/` directory.

```bash
beaver init --path <dir>
```

## Flags

| Flag | Effect |
|---|---|
| `-p, --path <dir>` | Where to create the scaffolded `beaver_config/`. |
| `-f, --force` | Overwrite an existing config (otherwise errors if the directory is non-empty). |
| `-d, --dev` | Populate with a development-friendly default config (sample Sigma rule, dashboard enabled, etc.). |

## What it produces

```
<dir>/beaver_config/
├── beaver_config.yaml          # stub — fill in project, region, input_subscription
├── sigma/
│   └── example_rule.yaml       # one sample Sigma rule (with --dev)
└── artifacts/                  # empty; populated on deploy
```

After `init`, edit `beaver_config.yaml` then run [`deploy`](./deploy.md).
