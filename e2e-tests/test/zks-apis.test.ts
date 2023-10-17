import { expect } from "chai";
import { deployContract, getTestProvider } from "../helpers/utils";
import * as hre from "hardhat";
import { Wallet } from "zksync-web3";
import { RichAccounts } from "../helpers/constants";
import { ethers } from "ethers";
import { TransactionRequest } from "zksync-web3/build/src/types";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";

const provider = getTestProvider();

interface Fee {
  gas_limit: ethers.BigNumber;
  gas_per_pubdata_limit: ethers.BigNumber;
  max_fee_per_gas: ethers.BigNumber;
  max_priority_fee_per_gas: ethers.BigNumber;
}

describe("zks_estimateFee", function () {
  it("Should return fee estimation data for transfer of 1 ETH", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const userWallet = Wallet.createRandom().connect(provider);
    const transaction: TransactionRequest = {
      from: wallet.address,
      to: userWallet.address,
      value: ethers.utils.parseEther("1")._hex,
    };

    // Act
    const response: Fee = await provider.send("zks_estimateFee", [transaction]);

    // Assert
    expect(ethers.BigNumber.from(response.gas_limit)).to.eql(ethers.BigNumber.from("1230957"), "Unexpected gas_limit");
    expect(ethers.BigNumber.from(response.gas_per_pubdata_limit)).to.eql(
      ethers.BigNumber.from("4080"),
      "Unexpected gas_per_pubdata_limit"
    );
    expect(ethers.BigNumber.from(response.max_fee_per_gas)).to.eql(
      ethers.BigNumber.from("250000000"),
      "Unexpected max_fee_per_gas"
    );
    expect(ethers.BigNumber.from(response.max_priority_fee_per_gas)).to.eql(
      ethers.BigNumber.from("0"),
      "Unexpected max_priority_fee_per_gas"
    );
  });
});

describe("zks_getTokenPrice", function () {
  it("Should return fake token Price for ETH", async function () {
    // Arrange
    const ethAddress = "0x0000000000000000000000000000000000000000";

    // Act
    const response: string = await provider.send("zks_getTokenPrice", [ethAddress]);

    // Assert
    expect(response).to.equal("1500");
  });
});

describe("zks_getTransactionDetails", function () {
  it("Should return transaction details for locally-executed transactions", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);

    const txReceipt = await greeter.setGreeting("Luke Skywalker");
    const details = await provider.send("zks_getTransactionDetails", [txReceipt.hash]);

    expect(details["status"]).to.equal("included");
    expect(details["initiatorAddress"].toLowerCase()).to.equal(wallet.address.toLowerCase());
  });
});

describe("zks_getBlockDetails", function () {
  it("Should return block details for locally-produced blocks", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);
    await greeter.setGreeting("Luke Skywalker");

    const latestBlock = await provider.getBlock("latest");
    const details = await provider.send("zks_getBlockDetails", [latestBlock.number]);

    expect(details["timestamp"]).to.equal(latestBlock.timestamp);
  });
});
