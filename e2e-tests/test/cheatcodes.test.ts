import { expect } from "chai";
import { Wallet } from "zksync-web3";
import * as hre from "hardhat";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import { RichAccounts } from "../helpers/constants";
import { deployContract, getTestProvider } from "../helpers/utils";

const provider = getTestProvider();

describe("Cheatcodes", function () {
  it("Should test vm.deal", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    const randomWallet = Wallet.createRandom().connect(provider);
    const initialBalance = await provider.getBalance(randomWallet.address);

    // Act
    const greeter = await deployContract(deployer, "TestCheatcodes", []);
    await greeter.deal(randomWallet.address, 123456, {
      gasLimit: 1000000,
    });

    // Assert
    const finalBalance = await provider.getBalance(randomWallet.address);
    expect(finalBalance.toNumber()).to.eq(123456);
    expect(finalBalance).to.not.eq(initialBalance);
  });

  it("Should test vm.etch", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    const randomWallet = Wallet.createRandom().connect(provider);
    const initialRandomWalletCode = await provider.getCode(randomWallet.address);

    // Act
    const cheatcodes = await deployContract(deployer, "TestCheatcodes", []);
    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);
    const greeterCode = await provider.getCode(greeter.address);
    await cheatcodes.etch(randomWallet.address, greeterCode);

    // Assert
    expect(initialRandomWalletCode).to.eq("0x");
    const finalRandomWalletCode = await provider.getCode(randomWallet.address);
    expect(finalRandomWalletCode).to.eq(greeterCode);
    expect(finalRandomWalletCode).to.not.eq(initialRandomWalletCode);
  });
});
