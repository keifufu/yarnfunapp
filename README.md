# yarnfunapp

<img alt="preview" src="./images/preview.gif">

# Usage

Either build from source or use the [Nix package manager](https://nixos.org/download/).

```bash
nix run github:keifufu/yarnfunapp
```

# Configuration

yarnfunapp is configured using optional environment variables.

| Variable                  | Default | Description                          |
| ------------------------- | ------- | ------------------------------------ |
| `YARN_OUTPUT`             | default | Monitor output, e.g. `DP-1`.         |
| `YARN_X`                  | `20`    | Horizontal offset.                   |
| `YARN_Y`                  | `20`    | Vertical offset.                     |
| `YARN_SIZE`               | `256`   | How large yarn will be.              |
| `YARN_IDLE_MS`            | `800`   | Time before returning to idle image. |
| `YARN_SWITCH_DEBOUNCE_MS` | `50`    | Minimum time between image switches. |
| `YARN_FLIP`               | `false` | Horizontally flips the image.        |
| `YARN_BOUNCE`             | `true`  | Enables the bounce animation.        |

# Credits

- Artwork commissioned by [@turniaxrun](https://x.com/turniptaxrun/status/2102291212690657315)
- Artwork created by [@x_ggi](https://x.com/x_ggi)
