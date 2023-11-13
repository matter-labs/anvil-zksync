import { expect } from "chai";
import { Wallet } from "zksync-web3";
import * as hre from "hardhat";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import { ethers } from "ethers";
import { RichAccounts } from "../helpers/constants";
import { deployContract, expectThrowsAsync, getTestProvider } from "../helpers/utils";
import { Log, TransactionReceipt } from "zksync-web3/build/src/types";

const provider = getTestProvider();

describe.only("Cheatcodes test", function () {
  it.only("should pass testDeal", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "TestCheatcodes", []);
    const testWallet = new Wallet(RichAccounts[1].PrivateKey);

    expect(await greeter.testDeal(testWallet.address, { gasLimit: 1_000_000 })).to.eq(true);
  });

  it.only("should pass testGetSetNonce", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "TestCheatcodes", []);
    const testWallet = new Wallet(RichAccounts[1].PrivateKey);

    expect(await greeter.testGetSetNonce(testWallet.address, { gasLimit: 1_000_000 })).to.eq(true);
  });
});
