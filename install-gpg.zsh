#!/usr/bin/env zsh

expected_gpg="${HOME}/.spm/runtime/bin/gpg"

if ! existing_gpg="$(command -v gpg)"; then
    echo "GPG not found. A pre-existing GPG installation is required to verify package signatures."
    exit 1
fi

if [[ "${existing_gpg}" = "${expected_gpg}" ]]; then
    echo "GPG is already installed at ${expected_gpg} via SPM."
    exit 0
fi

spm install npth 1.8
spm install libgpg-error 1.59
spm install libassuan 3.0.2
spm install libgcrypt 1.12.1
spm install libksba 1.6.8
spm install gnupg 2.5.18

if ! gpg="$(command -v "${expected_gpg}")"; then
    echo "GPG not found at ${expected_gpg} after installation."
    exit 1
fi
echo "GPG installed at ${gpg}."


if ! gpg="$(command -v gpg)"; then
    echo "GPG not found in PATH after installation."
    exit 1
fi

if [[ "${gpg}" = "${expected_gpg}" ]]; then
    echo "GPG at ${gpg} replaced ${existing_gpg}"
    exit 1
fi
