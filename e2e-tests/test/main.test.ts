import { expect } from "chai";
import { Wallet } from "zksync-ethers";
import * as hre from "hardhat";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import { ethers, Contract } from "ethers";
import { RichAccounts } from "../helpers/constants";
import { deployContract, expectThrowsAsync, getTestProvider } from "../helpers/utils";
import { Log, TransactionReceipt } from "zksync-ethers/build/types";

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
      const txResponse = await wallet.sendTransaction({
        to: userWallet.address,
        value: ethers.parseEther("3"),
      });
      await txResponse.wait();

      // deploy Greeter contract
      const artifact = await deployer.loadArtifact("Greeter");
      const greeter = await deployer.deploy(artifact, ["Hello, world!"]);

      // should revert
      const tx = await (greeter.connect(userWallet) as Contract).setGreetingByOwner("Hola, mundo!");
      await tx.wait();
    };

    await expectThrowsAsync(action, "Ownable: caller is not the owner");
  });

  it("Should produce event logs", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);
    const greeterAddress = await greeter.getAddress();

    expect(await greeter.greet()).to.eq("Hi");

    const setGreetingTx = await greeter.setGreeting("Luke Skywalker");
    const receipt: TransactionReceipt = await setGreetingTx.wait();

    // Validate log is created
    expect(receipt.logs.length).to.greaterThanOrEqual(1);
    const setGreetingLog = receipt.logs.find((log) => log.address === greeterAddress);
    expect(setGreetingLog).not.to.equal(null);

    const eventInterface = new ethers.Interface(["event LogString(string value)"]);
    const parsedLog = eventInterface.parseLog(setGreetingLog!);
    const parsedLogArg = parsedLog?.args[0].toString();
    expect(parsedLogArg).to.equal("Greeting is being updated to Luke Skywalker");
  });

  it("Should filter event logs", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);
    const greeterAddress = await greeter.getAddress();
    expect(await greeter.greet()).to.eq("Hi");

    const setGreetingTx = await greeter.setGreeting("Luke Skywalker");
    let receipt: TransactionReceipt = await setGreetingTx.wait();

    expect(receipt.logs.length).to.greaterThanOrEqual(1);
    const setGreetingLog = receipt.logs.find((log) => log.address === greeterAddress);
    expect(setGreetingLog).not.to.equal(null);

    // Create filter
    const topic = setGreetingLog!.topics[0];
    const filterId = await provider.send("eth_newFilter", [
      {
        fromBlock: "earliest",
        toBlock: "latest",
        topics: [topic],
      },
    ]);

    // New filter should be empty
    let filterChanges: Log[] = await provider.send("eth_getFilterChanges", [filterId]);
    expect(filterChanges).to.empty;

    // Emit logs and filter should not be empty
    receipt = await (await greeter.setGreeting("Darth Vader")).wait();
    filterChanges = await provider.send("eth_getFilterChanges", [filterId]);

    expect(filterChanges.length).to.eq(1);
    expect(filterChanges[0].transactionHash).to.eq(receipt.hash);
    expect(filterChanges[0].blockHash).to.eq(receipt.blockHash);
    expect(filterChanges[0].removed).to.eq(false);
    const eventInterface = new ethers.Interface(["event LogString(string value)"]);
    expect(eventInterface.parseLog(filterChanges[0])?.args[0]).to.equal("Greeting is being updated to Darth Vader");
  });

  it("Should filter new transactions", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);

    // Create filter
    const filterId = await provider.send("eth_newPendingTransactionFilter", []);

    // New filter should be empty
    let filterChanges: Log[] = await provider.send("eth_getFilterChanges", [filterId]);
    expect(filterChanges).to.empty;

    // Submit a tx and filter should contain it
    const setGreetingTx = await greeter.setGreeting("Luke Skywalker");
    const receipt: TransactionReceipt = await setGreetingTx.wait();
    filterChanges = await provider.send("eth_getFilterChanges", [filterId]);

    expect(filterChanges.length).to.eq(1);
    expect(filterChanges[0]).to.eq(receipt.hash);
  });
});
