# firefly-cardano


## Getting Set Up

Install Nix:
```console
# Install Nix
curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install

# Enter devshell:
nix develop
```

### (Optional) Install direnv
Should you prefer to load the devshell automatically when in a terminal

- Install Direnv:
  ```
  # Install direnv
  nix profile install nixpkgs#direnv

  # Configure your shell to load direnv everytime you enter this project (If you do not use bash see: https://direnv.net/docs/hook.html)
  echo 'eval "$(direnv hook bash)"' >> ~/.bashrc

  # And in case your system does not automatically .bashrc
  echo 'eval "$(direnv hook bash)"' >> ~/.bash_profile
  ```

- Renter the shell for direnv to take effect
- Trust direnv config (.envrc), whitelisting is required whenever this file changes
  ```
  direnv allow
  ```
