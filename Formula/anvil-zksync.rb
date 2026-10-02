class AnvilZksync < Formula
    desc "An in-memory ZKSync node for fast Elastic Network ZK chain development"
    homepage "https://github.com/matter-labs/anvil-zksync"
  version "0.7.1"
  
    on_macos do
      if Hardware::CPU.arm?
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-aarch64-apple-darwin.tar.gz"
        sha256 "8f37026e6f8338b0df512e5763decb6ee788294342c60e20b2b65ee301125f18"
      else
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-x86_64-apple-darwin.tar.gz"
        sha256 "abe3244996ed8c8d46882c22b0c34d15e350bc24957b412410d87e28fcebee85"
      end
    end
  
    on_linux do
      if Hardware::CPU.arm?
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
        sha256 "4ac4f58127452dae665367504ca71c09f41254fc62a017c50342d1307cd6b4d0"
      else
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
        sha256 "cfa467cc71396401ed56c45f4a32aee88262d0229dbec3057173180ba0cfbbc1"
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
