class AnvilZksync < Formula
    desc "An in-memory ZKSync node for fast Elastic Network ZK chain development"
    homepage "https://github.com/matter-labs/anvil-zksync"
  version "0.8.0"
  
    on_macos do
      if Hardware::CPU.arm?
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-aarch64-apple-darwin.tar.gz"
        sha256 "e41c7991b310b0a38cd84c75e7457f6309838087d0ab4881157cc4fb15638e46"
      else
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-x86_64-apple-darwin.tar.gz"
        sha256 "1c55d89316bc2a919c6240bc948657516816a8cce322f1f68ebb462cdeeff8b3"
      end
    end
  
    on_linux do
      if Hardware::CPU.arm?
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
        sha256 "9b95eda4a7b8c87ed545746fd169328cd4791e070a10d076a93b394ed5b7324e"
      else
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
        sha256 "5f8bb311d0a1da743ab5d9111a6230d5692e6f94f399c5c6fd918a42bb8b18ba"
      end
    end
  
    def install
      bin.install "anvil-zksync"
    end
  
    test do
      assert_match version, shell_output("#{bin}/anvil-zksync --version")
    end
  
    livecheck do
      url :stable
      regex(/^v?(\d+(?:\.\d+)+)$/i)
    end
  end
