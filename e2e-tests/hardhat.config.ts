import { HardhatUserConfig } from "hardhat/config";

import "@matterlabs/hardhat-zksync-deploy";
import "@matterlabs/hardhat-zksync-solc";
import "@nomiclabs/hardhat-ethers";

const config: HardhatUserConfig = {
  zksolc: {
    version: "latest",
    settings: {},
  },
  defaultNetwork: "zkSyncTestnet",
  networks: {
    zkSyncTestnet: {
      // Using 127.0.0.1 instead of localhost is necessary for CI builds
      url: "http://127.0.0.1:8011",
      // ethNetwork isn't necessary, but leaving for posterity
      ethNetwork: "http://127.0.0.1:8545",
      zksync: true,
    },
  },
  solidity: {
    version: "0.8.17",
  },
  mocha: {
    // Multiple reports allow view of the ouput in the console and as a JSON for the test result exporter in CI
    reporter: "mocha-multi",
    reporterOptions: {
      spec: "-",
      json: "test-results.json",
    },
  },
};

export default config;
