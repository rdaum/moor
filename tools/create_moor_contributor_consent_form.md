# Creating the mooR Contributor Consent Form

This repository includes a Google Apps Script for creating a Google Form that records retrospective
licensing consent from past mooR contributors.

Script file:

- `create_moor_contributor_consent_form.gs`

## How to run it

1. Go to <https://script.google.com>.
2. Create a new Apps Script project.
3. Replace the default `Code.gs` contents with the contents of `create_moor_contributor_consent_form.gs`.
4. Save the project.
5. In the function selector, choose `createMoorContributorConsentForm`.
6. Click **Run**.
7. Approve the requested Google permissions.
8. Open **View -> Logs** after the script runs.
9. Copy the logged form edit URL or published URL.

## What it creates

The script creates a Google Form titled `mooR Contributor Licensing Consent` with:

- contributor identity fields
- an authority-to-consent question
- a link to the current `CLA.md`
- an explicit consent statement
- typed-name and date fields for electronic signature
