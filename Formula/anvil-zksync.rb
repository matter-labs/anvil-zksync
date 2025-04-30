class AnvilZksync < Formula
    desc "An in-memory ZKSync node for fast Elastic Network ZK chain development"
    homepage "https://github.com/matter-labs/anvil-zksync"
    version "0.6.0"
  
    on_macos do
      if Hardware::CPU.arm?
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-aarch64-apple-darwin.tar.gz"
        sha256 "0019dfc4b32d63c1392aa264aed2253c1e0c2fb09216f8e2cc269bbfb8bb49b5"
      else
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-x86_64-apple-darwin.tar.gz"
        sha256 "0019dfc4b32d63c1392aa264aed2253c1e0c2fb09216f8e2cc269bbfb8bb49b5"
      end
    end
  
    on_linux do
      if Hardware::CPU.arm?
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
        sha256 "0019dfc4b32d63c1392aa264aed2253c1e0c2fb09216f8e2cc269bbfb8bb49b5"
      else
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
        sha256 "0019dfc4b32d63c1392aa264aed2253c1e0c2fb09216f8e2cc269bbfb8bb49b5"
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
