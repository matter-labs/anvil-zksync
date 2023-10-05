import { expect } from "chai";
import { Wallet } from "zksync-web3";
import * as hre from "hardhat";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import { RichAccounts } from "../helpers/constants";
import { deployContract, expectThrowsAsync, getTestProvider } from "../helpers/utils";

const provider = getTestProvider();

describe("debug namespace", function () {

    it("Should return error if block is not 'latest' or unspecified", async function () {
        expectThrowsAsync(async () => {
            await provider.send("debug_traceCall", [
                { to: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", },
                "earliest"]);
        }, "block parameter should be 'latest' or unspecified");

        expectThrowsAsync(async () => {
            await provider.send("debug_traceCall", [
                { to: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", },
                "1"]);
        }, "block parameter should be 'latest' or unspecified");
    });

    it("Should only trace top-level calls with onlyTopCall", async function () {
        const wallet = new Wallet(RichAccounts[0].PrivateKey);

        const deployer = new Deployer(hre, wallet);
        const secondary = await deployContract(deployer, "Secondary", ["3"]);
        const _primary = await deployContract(deployer, "Primary", [secondary.address]);

        const result = await provider.send("debug_traceCall", [
            {
                to: secondary.address,
                data: secondary.interface.encodeFunctionData("multiply", ["4"]),
                gas: "0x5f5e100",
            }, "latest", { tracer: "callTracer", tracerConfig: { onlyTopCall: true } }
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
            }
        ]);

        const { calls, output, revertReason } = result;

        // call should be successful
        expect(revertReason).to.equal(null);

        // subcall from primary to secondary contract should be present
        const contract_call = calls[0].calls.at(-1).calls[0].calls[0];
        expect(contract_call.from.toLowerCase()).to.equal(primary.address.toLowerCase());
        expect(contract_call.to.toLowerCase()).to.equal(secondary.address.toLowerCase());

        let [output_number] = primary.interface.decodeFunctionResult("calculate", output);
        expect(output_number.toNumber()).to.equal(12);
    });
});