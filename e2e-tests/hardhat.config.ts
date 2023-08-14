import { HardhatUserConfig } from "hardhat/config";

import "@matterlabs/hardhat-zksync-deploy";
import "@matterlabs/hardhat-zksync-solc";

const config: HardhatUserConfig = {
  zksolc: {
    version: "latest",
    settings: {},
  },
  defaultNetwork: "zkSyncTestnet",
  networks: {
    zkSyncTestnet: {
      url: "http://localhost:8011",
      // ethNetwork isn't necessary, but leaving for posterity
      ethNetwork: "http://localhost:8545",
      zksync: true,
    }
  },
  solidity: {
    version: "0.8.17",
  },
  mocha: {
    reporter: "mocha-junit-reporter"
  }
};

export default config;
