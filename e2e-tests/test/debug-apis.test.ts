import { expect } from "chai";
import { Wallet } from "zksync-web3";
import * as hre from "hardhat";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import { RichAccounts } from "../helpers/constants";
import { deployContract, expectThrowsAsync, getTestProvider } from "../helpers/utils";
import { BigNumber } from "ethers";

const provider = getTestProvider();

describe("debug_traceCall", function () {
  it("Should return error if block is not 'latest' or unspecified", async function () {
    expectThrowsAsync(async () => {
      await provider.send("debug_traceCall", [{ to: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266" }, "earliest"]);
    }, "block parameter should be 'latest' or unspecified");

    expectThrowsAsync(async () => {
      await provider.send("debug_traceCall", [{ to: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266" }, "1"]);
    }, "block parameter should be 'latest' or unspecified");
  });

  it("Should only trace top-level calls with onlyTopCall", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);

    const deployer = new Deployer(hre, wallet);
    const secondary = await deployContract(deployer, "Secondary", ["3"]);
    await deployContract(deployer, "Primary", [secondary.address]);

    const result = await provider.send("debug_traceCall", [
      {
        to: secondary.address,
        data: secondary.interface.encodeFunctionData("multiply", ["4"]),
        gas: "0x5f5e100",
      },
      "latest",
      { tracer: "callTracer", tracerConfig: { onlyTopCall: true } },
    ]);

    const { calls, revertReason } = result;

    // call should be successful
    expect(revertReason).to.equal(null);

    // should have no subcalls
    expect(calls.length).to.equal(0);
  });

  it("Should trace contract calls", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);

    const deployer = new Deployer(hre, wallet);
    const secondary = await deployContract(deployer, "Secondary", ["3"]);
    const primary = await deployContract(deployer, "Primary", [secondary.address]);

    const result = await provider.send("debug_traceCall", [
      {
        to: primary.address,
        data: primary.interface.encodeFunctionData("calculate", ["4"]),
        gas: "0x5f5e100",
      },
    ]);

    const { calls, output, revertReason } = result;

    // call should be successful
    expect(revertReason).to.equal(null);

    // subcall from primary to secondary contract should be present
    const contract_call = calls[0].calls.at(-1).calls[0].calls[0];
    expect(contract_call.from.toLowerCase()).to.equal(primary.address.toLowerCase());
    expect(contract_call.to.toLowerCase()).to.equal(secondary.address.toLowerCase());

    const [output_number] = primary.interface.decodeFunctionResult("calculate", output);
    expect(output_number.toNumber()).to.equal(12);
  });
});

describe("debug_traceTransaction", function () {
  it("Should return null if txn hash is unknown", async function () {
    const result = await provider.send("debug_traceTransaction", [
      "0xd3a94ff697a573cb174ecce05126e952ecea6dee051526a3e389747ff86b0d99",
    ]);
    expect(result).to.equal(null);
  });

  it("Should trace prior transactions", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);

    const txReceipt = await greeter.setGreeting("Luke Skywalker");
    const trace = await provider.send("debug_traceTransaction", [txReceipt.hash]);

    // call should be successful
    expect(trace.error).to.equal(null);
    expect(trace.calls.length).to.equal(1);

    // gas limit should match
    expect(BigNumber.from(trace.gas).toNumber()).to.equal(txReceipt.gasLimit.toNumber());
  });

  it("Should respect only_top_calls option", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);

    const txReceipt = await greeter.setGreeting("Luke Skywalker");
    const trace = await provider.send("debug_traceTransaction", [
      txReceipt.hash,
      { tracer: "callTracer", tracerConfig: { onlyTopCall: true } },
    ]);

    // call should be successful
    expect(trace.error).to.equal(null);
    expect(trace.calls.length).to.equal(0);
  });
});

describe("debug_traceBlockByHash", function () {
  it("Should trace prior blocks", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);

    const txReceipt = await greeter.setGreeting("Luke Skywalker");
    const latestBlock = await provider.getBlock("latest");
    const block = await provider.getBlock(latestBlock.number - 1);

    const traces = await provider.send("debug_traceBlockByHash", [block.hash]);

    // block should have 1 traces
    expect(traces.length).to.equal(1);

    // should contain trace for our tx
    const trace = traces[0].result;
    expect(trace.input).to.equal(txReceipt.data);
  });
});

describe("debug_traceBlockByNumber", function () {
  it("Should trace prior blocks", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);

    const txReceipt = await greeter.setGreeting("Luke Skywalker");

    // latest block will be empty, check we get no traces for it
    const empty_traces = await provider.send("debug_traceBlockByNumber", ["latest"]);
    expect(empty_traces.length).to.equal(0);

    // latest - 1 should contain our traces
    const latestBlock = await provider.getBlock("latest");
    const traces = await provider.send("debug_traceBlockByNumber", [(latestBlock.number - 1).toString(16)]);

    // block should have 1 traces
    expect(traces.length).to.equal(1);

    // should contain trace for our tx
    const trace = traces[0].result;
    expect(trace.input).to.equal(txReceipt.data);
  });
});
