class AnvilZksync < Formula
    desc "An in-memory ZKSync node for fast Elastic Network ZK chain development"
    homepage "https://github.com/matter-labs/anvil-zksync"
  version "0.6.9"
  
    on_macos do
      if Hardware::CPU.arm?
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-aarch64-apple-darwin.tar.gz"
        sha256 "b05aed9dae4dbaabe8bb5ab03de37ce3d4c16786728deca07aca3979036ba3a7"
      else
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-x86_64-apple-darwin.tar.gz"
        sha256 "517717eafbd521b396ab9f6e59c6a8bb9697a07d056fe034e776ceb3dd314c14"
      end
    end
  
    on_linux do
      if Hardware::CPU.arm?
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
        sha256 "652e0b1e55edd390297de4f409c1d69d9e31f4e9dbf82390175671fabf8ff4cd"
      else
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
        sha256 "3baaaa79686cff26a645a1efce1d2511dd78925dc79757a774937c9c491f9536"
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
