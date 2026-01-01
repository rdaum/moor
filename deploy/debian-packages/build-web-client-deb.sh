#!/bin/bash
# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
# software: you can redistribute it and/or modify it under the terms of the GNU
# General Public License as published by the Free Software Foundation, version
# 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License along with
# this program. If not, see <https://www.gnu.org/licenses/>.
#

set -e

# Build script for moor-web-client debian package
# This script creates a debian package containing the built web client static files

VERSION="1.0.0~beta1-1"
PACKAGE_NAME="moor-web-client"
BUILD_DIR="debian-pkg-web-client"

echo "Building ${PACKAGE_NAME} ${VERSION}..."

# Clean up any previous build
rm -rf ${BUILD_DIR}
rm -f ${PACKAGE_NAME}_${VERSION}_all.deb

# Create package directory structure
mkdir -p ${BUILD_DIR}/DEBIAN
mkdir -p ${BUILD_DIR}/usr/share/moor/web-client
mkdir -p ${BUILD_DIR}/usr/share/doc/${PACKAGE_NAME}

# Copy built web client files
echo "Copying web client files from dist/..."
cp -r dist/* ${BUILD_DIR}/usr/share/moor/web-client/

# Copy documentation
cp deploy/debian-packages/nginx-for-debian.conf ${BUILD_DIR}/usr/share/doc/${PACKAGE_NAME}/

# Create control file
cat > ${BUILD_DIR}/DEBIAN/control <<EOF
Package: ${PACKAGE_NAME}
Version: ${VERSION}
Architecture: all
Maintainer: Ryan Daum <ryan.daum@gmail.com>
Description: Web client for mooR
 Modern web client for mooR - a 21st century LambdaMOO implementation.
 This package contains the built static files that can be served by any
 web server (nginx, caddy, apache, etc).
Section: web
Priority: optional
Recommends: nginx
EOF

# Build the package
echo "Building debian package..."
dpkg-deb --root-owner-group --build ${BUILD_DIR} target/debian/${PACKAGE_NAME}_${VERSION}_all.deb

# Sign the package if GPG_KEY_ID is set
if [ -n "$GPG_KEY_ID" ]; then
  echo "Signing package with GPG key: $GPG_KEY_ID"
  # Install dpkg-sig if not available
  if ! command -v dpkg-sig &> /dev/null; then
    echo "Installing dpkg-sig..."
    apt-get update && apt-get install -y dpkg-sig
  fi
  dpkg-sig --sign builder -k "$GPG_KEY_ID" target/debian/${PACKAGE_NAME}_${VERSION}_all.deb
  echo "Package signed successfully"
else
  echo "No GPG_KEY_ID set, skipping package signing"
fi

echo "Package built successfully: target/debian/${PACKAGE_NAME}_${VERSION}_all.deb"

# Clean up
rm -rf ${BUILD_DIR}
