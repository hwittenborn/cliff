set positional-arguments

build:
	cargo build --release

install:
	install -Dm 755 target/release/celeste "{{ env_var('DESTDIR') }}/usr/bin/celeste"
	install -Dm 644 assets/com.hunterwittenborn.Celeste.desktop "{{ env_var('DESTDIR') }}/usr/share/applications/com.hunterwittenborn.Celeste.desktop"
	install -Dm 644 assets/com.hunterwittenborn.Celeste-regular.svg "{{ env_var('DESTDIR') }}/usr/share/icons/hicolor/scalable/apps/com.hunterwittenborn.Celeste.svg"
	install -Dm 644 assets/context/com.hunterwittenborn.Celeste.CelesteTrayLoading-symbolic.svg "{{ env_var('DESTDIR') }}/usr/share/icons/hicolor/symbolic/apps/com.hunterwittenborn.Celeste.CelesteTrayLoading-symbolic.svg"
	install -Dm 644 assets/context/com.hunterwittenborn.Celeste.CelesteTraySyncing-symbolic.svg "{{ env_var('DESTDIR') }}/usr/share/icons/hicolor/symbolic/apps/com.hunterwittenborn.Celeste.CelesteTraySyncing-symbolic.svg"
	install -Dm 644 assets/context/com.hunterwittenborn.Celeste.CelesteTrayWarning-symbolic.svg "{{ env_var('DESTDIR') }}/usr/share/icons/hicolor/symbolic/apps/com.hunterwittenborn.Celeste.CelesteTrayWarning-symbolic.svg"
	install -Dm 644 assets/context/com.hunterwittenborn.Celeste.CelesteTrayDone-symbolic.svg "{{ env_var('DESTDIR') }}/usr/share/icons/hicolor/symbolic/apps/com.hunterwittenborn.Celeste.CelesteTrayDone-symbolic.svg"
	install -Dm 644 assets/com.hunterwittenborn.Celeste.metainfo.xml "{{ env_var('DESTDIR') }}/usr/share/metainfo/com.hunterwittenborn.Celeste.metainfo.xml"

clippy:
	cargo clippy -- -D warnings

fmt:
    cargo +nightly fmt

get-version:
    #!/usr/bin/env bash
    source makedeb/PKGBUILD
    echo "${pkgver}"

update-versions:
    #!/usr/bin/env bash
    set -euo pipefail
    version="$(just get-version)"
    sed -i "s|^version = .*|version = \"${version}\"|" Cargo.toml

    date="$(cat CHANGELOG.md | grep "^## \[${version}\]" | grep -o '[^ ]*$')"
    notes="$(parse-changelog CHANGELOG.md "${version}")"
    just update-metainfo

update-metainfo:
    #!/usr/bin/env python3
    import sys
    import markdown
    import bs4
    import re

    from bs4 import BeautifulSoup, NavigableString
    from bs4.formatter import HTMLFormatter

    # Parse the metainfo and Changelog into BeautifulSoup objects. 
    metainfo_path = "assets/com.hunterwittenborn.Celeste.metainfo.xml"
    changelog_path = "CHANGELOG.md"
    
    text = open(metainfo_path).read()
    changelog_md = open(changelog_path).read()
    changelog_html = markdown.markdown(changelog_md)
    
    soup = BeautifulSoup(text, features="xml")
    changelog_soup = BeautifulSoup(changelog_html, "html.parser")
    releases = []
    
    # The changelog is in a flat list of HTML tags. Group them into dicts of
    # `version: [html-elements]` for easier usage.
    versions = {}
    current_version = None

    for tag in changelog_soup:
        # We don't need empty lines, so ignore them.
        if tag.text == "\n":
            continue
        # Version tags are '##' in markdown (i.e. an '<h2>').
        elif tag.name == "h2" and tag.text != "[Unreleased]":
            version = re.search("[^[][0-9.]*", tag.text).group(0)
            date = re.search("[^ ]*$", tag.text).group(0)

            release_soup = BeautifulSoup("<release><description></description></release>", "html.parser").release
            release_soup["date"] = date
            release_soup["version"] = version

            versions[version] = release_soup
            current_version = version
        # If we aren't on a version tag and haven't gotten any version yet,
        # we're dealing with content before the first version tag and we
        # should ignore it.
        elif len(versions) == 0:
            continue
        # Appstream doesn't support headers in descriptions, so format them as `<p>` tags.
        elif tag.name == "h3":
            match tag.text:
                case "Added":
                    header = "New features in this release:"
                case "Changed":
                    header = "Changes in this release:"
                case "Fixed":
                    header = "Fixes in this release:"
                case _:
                    raise Exception(f"Unknown change type: `{tag.text}`")
            
            versions[version].description.append(BeautifulSoup(f"<p>{header}</p>", "html.parser"))
        # Otherwise we're adding to the existing version.
        else:
            versions[version].description.append(tag)

    # Clear out the existing versions and write the new ones.
    soup.component.releases.clear()
    
    for release in versions.values():
        soup.component.releases.append(release)

    output = soup.prettify(formatter=HTMLFormatter(indent=4))
    open(metainfo_path, "w").write(output)

update-translations:
    xtr src/main.rs --copyright-holder 'Hunter Wittenborn <hunter@hunterwittenborn.com>' -o /dev/stdout --package-name 'Celeste' --package-version "$(just get-version)" > po/com.hunterwittenborn.Celeste.pot

# Create the Snap using an already build copy of Celeste. This currently requires you to be running on Ubuntu 22.10 or newer.
create-host-snap:
    #!/usr/bin/env bash
    set -euo pipefail

    cd "$(git rev-parse --show-toplevel)"
    host_snapcraft_yml="$(cat snap/snapcraft.yaml | grep -Ev 'source-type: |override-build: |just build')"
    tmpdir="$(mktemp -d)"

    find ./ -mindepth 1 -maxdepth 1 -not -path './target' -exec cp '{}' "${tmpdir}/{}" -R \;
    mkdir -p "${tmpdir}/target/release"
    cp target/debug/celeste "${tmpdir}/target/release"

    cd "${tmpdir}"
    echo "${host_snapcraft_yml}" > snap/snapcraft.yaml
    snapcraft -v

    cd -
    cp "${tmpdir}/"*.snap ./
    rm "${tmpdir}" -rf
