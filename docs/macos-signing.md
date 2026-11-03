# Signing and notarizing the macOS app

Without signing, macOS warns that the developer cannot be verified. With a **Developer ID Application**
certificate and notarization the app opens without any warning. The release workflow does this
automatically once the six repository secrets below exist.

## 1. Create a Developer ID Application certificate

Only the **Account Holder** of the Apple Developer team can create this certificate type.

Easiest, with Xcode: *Xcode → Settings → Accounts →* select your Apple ID *→ Manage Certificates… → + →
Developer ID Application*.

Without Xcode:

1. *Keychain Access → Certificate Assistant → Request a Certificate From a Certificate Authority…*. Enter your
   email address, choose *Saved to disk*, and save the `.certSigningRequest` file.
2. On <https://developer.apple.com/account/resources/certificates/add> choose **Developer ID Application**
   (G2 Sub-CA), upload the request, download the `.cer` file and double-click it to add it to your keychain.

## 2. Export the certificate as .p12

1. In *Keychain Access → login → My Certificates*, find **Developer ID Application: Your Name (TEAMID)**.
   Expand it to check that a private key is attached.
2. Right-click → *Export…* → format *Personal Information Exchange (.p12)*. Choose a strong password.
3. Encode it for GitHub:

   ```bash
   base64 -i DeveloperID.p12 | tr -d '\n' > DeveloperID.p12.base64
   ```

4. Look up the exact identity name:

   ```bash
   security find-identity -v -p codesigning
   ```

   Copy the quoted text, for example `Developer ID Application: Jane Doe (AB12CD34EF)`.

## 3. Create an app-specific password (for notarization)

On <https://account.apple.com> → *Sign-In and Security → App-Specific Passwords* → create one named
`2DriveScan notarization`. Your **Team ID** (10 characters) is on
<https://developer.apple.com/account> → *Membership details*.

## 4. Add the repository secrets

GitHub → repository → *Settings → Secrets and variables → Actions → New repository secret*:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | contents of `DeveloperID.p12.base64` |
| `APPLE_CERTIFICATE_PASSWORD` | the .p12 export password |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Jane Doe (AB12CD34EF)` |
| `APPLE_ID` | your Apple Account email address |
| `APPLE_PASSWORD` | the app-specific password |
| `APPLE_TEAM_ID` | your 10-character Team ID |

Or with the GitHub CLI, from the folder with the files:

```bash
gh secret set APPLE_CERTIFICATE --repo joostiphone/2drivescan < DeveloperID.p12.base64
gh secret set APPLE_CERTIFICATE_PASSWORD --repo joostiphone/2drivescan
gh secret set APPLE_SIGNING_IDENTITY --repo joostiphone/2drivescan
gh secret set APPLE_ID --repo joostiphone/2drivescan
gh secret set APPLE_PASSWORD --repo joostiphone/2drivescan
gh secret set APPLE_TEAM_ID --repo joostiphone/2drivescan
```

(`gh secret set` without `<` asks for the value interactively, so it never ends up in your shell history.)

Afterwards, delete the `.p12` and `.base64` files from your disk or store them in a password manager.

## 5. Release

The next release (tag or *Actions → Release → Run workflow*) is signed and notarized. The build log of the
macOS job shows "Signing with a Developer ID certificate" and "Notarization enabled".
