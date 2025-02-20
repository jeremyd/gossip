
Prerequisite for packaging:

* You need Wix 4 tools installed, probably with DOTNET installed first.

Compile:

  $ rustup update
  $ cargo build --features=lang-cjk --release

Copy the binary to the packaging diretory

  $ cp ..\..\target\release\gossip.exe .

Copy the gossip.png here

  $ cp ..\..\gossip.png .

For new versions of gossip, update gossip.wxs
    * UPDATE the Package.Version, SummaryInformation.Description
	* UPDATE the Package.ProductCode GUID to a new one
    * KEEP the UpgradeCode GUID (it should never change, it ties different versions together)
    * Change a component GUID ONLY IF the absolute path changes.

Packaging:

  $ wix build gossip.VERSION.wxs

Upload to github releases.


----
To install the package, either double-click the MSI, or

  $ msiexec gossip.msi

To remove the package from your windows computer:

  $ msiexec /x gossip.msi
