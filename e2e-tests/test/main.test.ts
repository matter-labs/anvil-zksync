import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import { expect } from "chai";
import * as hre from "hardhat";
import { Wallet } from "zksync-web3";
import { RichAccounts } from "../helpers/constants";
import { deployContract } from "../helpers/utils";

describe("Greeter", function () {
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
});
