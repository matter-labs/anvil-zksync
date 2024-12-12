import { expect } from "chai";
import { deployContract, getTestProvider } from "../helpers/utils";
import { Wallet } from "zksync-ethers";
import { RichAccounts } from "../helpers/constants";
import { ethers } from "ethers";
import * as hre from "hardhat";
import { TransactionRequest } from "zksync-ethers/build/types";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import { TransactionResponse } from "@ethersproject/abstract-provider";

const provider = getTestProvider();

interface Fee {
  gas_limit: bigint;
  gas_per_pubdata_limit: bigint;
  max_fee_per_gas: bigint;
  max_priority_fee_per_gas: bigint;
}

describe("zks_estimateFee", function () {
  it("Should return valid fee estimation data for transfer of 1 ETH", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const userWallet = Wallet.createRandom().connect(provider);
    const transaction: TransactionRequest = {
      from: wallet.address,
      to: userWallet.address,
      value: ethers.toBeHex(ethers.parseEther("1")),
    };

    // Act
    const response: Fee = await provider.send("zks_estimateFee", [transaction]);

    // Assert
    expect(response).to.have.property("gas_limit");
    expect(response).to.have.property("gas_per_pubdata_limit");
    expect(response).to.have.property("max_fee_per_gas");
    expect(response).to.have.property("max_priority_fee_per_gas");

    const gasLimit = BigInt(response.gas_limit);
    const gasPerPubdataLimit = BigInt(response.gas_per_pubdata_limit);
    const maxFeePerGas = BigInt(response.max_fee_per_gas);

    expect(gasLimit).to.be.greaterThan(0, "gas_limit should be greater than 0");
    expect(gasPerPubdataLimit).to.be.greaterThan(0, "gas_per_pubdata_limit should be greater than 0");
    expect(maxFeePerGas).to.be.greaterThan(0, "max_fee_per_gas should be greater than 0");
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

    const txResponse: TransactionResponse = await greeter.setGreeting("Luke Skywalker");
    const txReceipt = await txResponse.wait();
    const details = await provider.send("zks_getTransactionDetails", [txReceipt.transactionHash]);

    expect(details["status"]).to.equal("included");
    expect(details["initiatorAddress"].toLowerCase()).to.equal(wallet.address.toLowerCase());
  });
});

describe("zks_getBridgeContracts", function () {
  it("Should return default values", async function () {
    const bridgeAddresses = await provider.send("zks_getBridgeContracts", []);

    expect(bridgeAddresses).to.deep.equal({
      l1Erc20DefaultBridge: null,
      l1SharedDefaultBridge: null,
      l2Erc20DefaultBridge: null,
      l2SharedDefaultBridge: null,
      l2LegacySharedBridge: null,
      l1WethBridge: null,
      l2WethBridge: null,
    });
  });
});

describe("zks_getBlockDetails", function () {
  it("Should return block details for locally-produced blocks", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);
    await (await greeter.setGreeting("Luke Skywalker")).wait();

    const latestBlock = await provider.getBlock("latest");
    const details = await provider.send("zks_getBlockDetails", [latestBlock.number]);

    expect(details["timestamp"]).to.equal(latestBlock.timestamp);
  });
});

describe("zks_getBytecodeByHash", function () {
  it("Should fetch the stored bytecode at address", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    const artifact = await deployer.loadArtifact("Greeter");
    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);
    expect(await greeter.greet()).to.eq("Hi");

    // get the bytecode hash from the event
    const contractDeployedHash = ethers
      .keccak256(ethers.toUtf8Bytes("ContractDeployed(address,bytes32,address)"))
      .substring(2);
    const logs = await provider.send("eth_getLogs", [
      {
        fromBlock: ethers.toBeHex(greeter.deploymentTransaction()!.blockNumber!),
        toBlock: ethers.toBeHex(greeter.deploymentTransaction()!.blockNumber!),
        address: "0x0000000000000000000000000000000000008006", // L2 Deployer address
        topics: [contractDeployedHash],
      },
    ]);
    expect(logs).to.not.be.empty;
    expect(logs[0].topics).to.have.lengthOf(4);
    const bytecodeHash = logs[0].topics[2];

    // Act
    const bytecode = await provider.send("zks_getBytecodeByHash", [bytecodeHash]);

    // Assert
    expect(ethers.hexlify(bytecode)).to.equal(artifact.deployedBytecode);
  });
});

describe("zks_getRawBlockTransactions", function () {
  it("Should return transactions for locally-produced blocks", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);
    const txResponse: TransactionResponse = await greeter.setGreeting("Luke Skywalker");
    await txResponse.wait();

    const latestBlock = await provider.getBlock("latest");
    const txns = await provider.send("zks_getRawBlockTransactions", [latestBlock.number - 1]);

    expect(txns.length).to.equal(1);
    expect(txns[0]["execute"]["calldata"]).to.equal(txResponse.data);
  });
});

describe("zks_getConfirmedTokens", function () {
  it("Should return only Ether", async function () {
    const tokens = await provider.send("zks_getConfirmedTokens", [0, 100]);
    expect(tokens.length).to.equal(1);
    expect(tokens[0].name).to.equal("Ether");
  });
});

describe("zks_getAllAccountBalances", function () {
  it("Should return balance of a rich account", async function () {
    // Arrange
    const account = RichAccounts[0].Account;
    const expectedBalance = ethers.parseEther("1000000000000"); // 1_000_000_000_000 ETH
    const ethAddress = "0x000000000000000000000000000000000000800a";
    await provider.send("hardhat_setBalance", [account, ethers.toBeHex(expectedBalance)]);

    // Act
    const balances = await provider.send("zks_getAllAccountBalances", [account]);
    const ethBalance = BigInt(balances[ethAddress]);

    // Assert
    expect(ethBalance === expectedBalance).to.be.true;
  });
});

describe("zks_getBaseTokenL1Address", function () {
  it("Should return 0x1 address", async function () {
    const token_address = await provider.send("zks_getBaseTokenL1Address", []);
    expect(token_address).to.equal("0x0000000000000000000000000000000000000001");
  });
});
