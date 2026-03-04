configure:
    mkdir -p "${HOME}/.config/spm"
    echo "[registry]" > "${HOME}/.config/spm/config.toml"
    echo "url = \"file:///{{justfile_directory()}}/registry\"" >> "${HOME}/.config/spm/config.toml"
