#!/usr/bin/env bash
# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
# software: you can redistribute it and/or modify it under the terms of the GNU
# Affero General Public License as published by the Free Software Foundation,
# version 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
# details.
#
# You should have received a copy of the GNU Affero General Public License along
# with this program. If not, see <https://www.gnu.org/licenses/>.

# Explicit Debian/Ubuntu provisioning for PostgreSQL-enabled builds and images.
set -euo pipefail
mode=${1:-build}
arch=${2:-$(dpkg --print-architecture)}
client=${3:-system}
case "$mode" in build|runtime) ;; *) echo 'Expected build or runtime' >&2; exit 2;; esac
case "$client" in system|17|18) ;; *) echo 'Expected system, 17, or 18 client' >&2; exit 2;; esac
minimum=17
packages=("libpq5:$arch")
if [[ "$mode" == build ]]; then packages+=("libpq-dev:$arch"); fi

# Ordinary provisioning retains a compatible installed library. Exact major versions
# are only for explicit compatibility jobs, which can intentionally downgrade.
if [[ "$client" == system ]]; then
    installed=true
    for package in "${packages[@]}"; do
        version=$(dpkg-query -W -f='${db:Status-Status} ${Version}' "$package" 2>/dev/null || true)
        if [[ "$version" != installed\ * ]] || ! dpkg --compare-versions "${version#installed }" ge "$minimum"; then
            installed=false
        fi
    done
    if [[ "$installed" == true ]]; then exit 0; fi
    apt-get update
    compatible=true
    for package in "${packages[@]}"; do
        candidate=$(apt-cache policy "$package" | awk '/Candidate:/ {print $2}')
        if [[ -z "$candidate" || "$candidate" == '(none)' ]] || ! dpkg --compare-versions "$candidate" ge "$minimum"; then
            compatible=false
        fi
    done
    if [[ "$compatible" == true ]]; then
        apt-get install -y --no-install-recommends "${packages[@]}"
        exit 0
    fi
fi

# Add PGDG only when the distribution cannot supply a compatible client, or when
# an explicit compatibility job needs its version-specific component.
# shellcheck source=/dev/null
. /etc/os-release
components=main
if [[ "$client" != system ]]; then components+=" $client"; fi
apt-get update
apt-get install -y --no-install-recommends ca-certificates curl
# Reuse the signing key of an existing PGDG source. APT rejects duplicate source
# entries with different Signed-By paths, even when both files contain the same key.
shopt -s nullglob
source_files=(/etc/apt/sources.list.d/*.list /etc/apt/sources.list.d/*.sources)
if [[ -f /etc/apt/sources.list ]]; then source_files+=(/etc/apt/sources.list); fi
signed_by=$(awk '
    BEGIN { RS="" }
    /apt.postgresql.org\/pub\/repos\/apt/ {
        for (i=1; i<=NF; i++) {
            if ($i == "Signed-By:") { print $(i+1); exit }
            if ($i ~ /^signed-by=/) { sub(/^signed-by=/,"",$i); sub(/].*$/,"",$i); print $i; exit }
        }
    }
' "${source_files[@]}" /dev/null)
if [[ -z "$signed_by" ]]; then
    signed_by=/usr/share/postgresql-common/pgdg/apt.postgresql.org.asc
    install -d /usr/share/postgresql-common/pgdg
    curl --fail --silent --show-error --location \
        https://www.postgresql.org/media/keys/ACCC4CF8.asc -o "$signed_by"
fi
cat >/etc/apt/sources.list.d/moor-pgdg.sources <<SOURCE
Types: deb
URIs: https://apt.postgresql.org/pub/repos/apt
Suites: ${VERSION_CODENAME}-pgdg
Components: $components
Architectures: $arch
Signed-By: $signed_by
SOURCE
apt-get update
if [[ "$client" == system ]]; then
    apt-get install -y --no-install-recommends "${packages[@]}"
else
    pinned=()
    for package in "${packages[@]}"; do pinned+=("$package=$client.*"); done
    apt-get install -y --no-install-recommends --allow-downgrades "${pinned[@]}"
fi
for package in "${packages[@]}"; do
    version=$(dpkg-query -W -f='${Version}' "$package")
    dpkg --compare-versions "$version" ge "$minimum"
    if [[ "$client" != system ]]; then [[ "$version" == "$client".* ]]; fi
done
