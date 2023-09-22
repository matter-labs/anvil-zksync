import { expect } from "chai";
import { Wallet } from "zksync-web3";
import * as hre from "hardhat";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import { ethers } from "ethers";
import { RichAccounts } from "../helpers/constants";
import {
  deployContract,
  expectThrowsAsync,
  getTestProvider,
} from "../helpers/utils";

const provider = getTestProvider();

describe("Greeter Smart Contract", function () {
  it("Should return the new greeting once it's changed", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);

    expect(await greeter.greet()).to.eq("Hi");

    const setGreetingTx = await greeter.setGreeting("Hola, mundo!");
    // wait until the transaction is mined
    await setGreetingTx.wait();

    expect(await greeter.greet()).to.equal("Hola, mundo!");
  });

  it("should prevent non-owners from setting greeting", async function () {
    const action = async () => {
      const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
      const deployer = new Deployer(hre, wallet);

      // setup user wallet with 3 ETH
      const userWallet = Wallet.createRandom().connect(provider);
      await wallet.sendTransaction({
        to: userWallet.address,
        value: ethers.utils.parseEther("3"),
      });

      // deploy Greeter contract
      const artifact = await deployer.loadArtifact("Greeter");
      const greeter = await deployer.deploy(artifact, ["Hello, world!"]);

      // should revert
      const tx = await greeter.connect(userWallet).setGreeting("Hola, mundo!");
      await tx.wait();
    };

    await expectThrowsAsync(action, "Ownable: caller is not the owner");
  });
});
