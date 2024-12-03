# Release

_These instructions are primarily intended for maintainers of this project_.

Releases containing binary artifacts are published on GitHub. The `release.sh` script automates the release process, which assumes that [GitHub CLI](https://cli.github.com/) has been installed.

The version of the release is derived from the package version in `Cargo.toml`. A new release on GitHub generates a corresponding tag, so the assumption is that the version number has been appropriately incremented. Otherwise, the release creation process will fail.

If the release process is successful, a new tag of the format `v<version>` is automatically created. For example, if the package version in `Cargo.toml` is `0.1.0`, then the corresponding tag is `v0.1.0`.

If the release process was problematic in any way, it can be deleted using the following command.

```shell
gh release delete --cleanup-tag --yes <tag>
```
