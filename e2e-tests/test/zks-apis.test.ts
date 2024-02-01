import { expect } from "chai";
import { deployContract, getTestProvider } from "../helpers/utils";
import { Wallet } from "zksync-web3";
import { RichAccounts } from "../helpers/constants";
import { BigNumber, ethers } from "ethers";
import * as hre from "hardhat";
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
    expect(ethers.BigNumber.from(response.gas_limit)).to.eql(ethers.BigNumber.from("621166"), "Unexpected gas_limit");
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

describe("zks_getBridgeContracts", function () {
  it("Should return default values", async function () {
    const bridgeAddresses = await provider.send("zks_getBridgeContracts", []);

    expect(bridgeAddresses).to.deep.equal({
      l1Erc20DefaultBridge: "0x0000000000000000000000000000000000000000",
      l2Erc20DefaultBridge: "0x0000000000000000000000000000000000000000",
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
    await greeter.setGreeting("Luke Skywalker");

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
    const deployedContract = await greeter.deployTransaction.wait();
    expect(await greeter.greet()).to.eq("Hi");

    // get the bytecode hash from the event
    const contractDeployedHash = ethers.utils
      .keccak256(ethers.utils.toUtf8Bytes("ContractDeployed(address,bytes32,address)"))
      .substring(2);
    const logs = await provider.send("eth_getLogs", [
      {
        fromBlock: ethers.utils.hexlify(deployedContract.blockNumber),
        toBlock: ethers.utils.hexlify(deployedContract.blockNumber),
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
    expect(ethers.utils.hexlify(bytecode)).to.equal(artifact.deployedBytecode);
  });
});

describe("zks_getRawBlockTransactions", function () {
  it("Should return transactions for locally-produced blocks", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);
    const receipt = await greeter.setGreeting("Luke Skywalker");

    const latestBlock = await provider.getBlock("latest");
    const txns = await provider.send("zks_getRawBlockTransactions", [latestBlock.number - 1]);

    expect(txns.length).to.equal(1);
    expect(txns[0]["execute"]["calldata"]).to.equal(receipt.data);
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
    const expectedBalance = ethers.utils.parseEther("1000000000000"); // 1_000_000_000_000 ETH
    const ethAddress = "0x000000000000000000000000000000000000800a";
    await provider.send("hardhat_setBalance", [account, expectedBalance._hex]);

    // Act
    const balances = await provider.send("zks_getAllAccountBalances", [account]);
    const ethBalance = BigNumber.from(balances[ethAddress]);

    // Assert
    expect(ethBalance.eq(expectedBalance)).to.be.true;
  });
});
