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


    it("Should trace top-level calls", async function () {
        const wallet = new Wallet(RichAccounts[0].PrivateKey);

        const deployer = new Deployer(hre, wallet);
        const secondary = await deployContract(deployer, "Secondary", ["3"]);
        const primary = await deployContract(deployer, "Primary", [secondary.address]);

        const data = secondary.interface.encodeFunctionData("multiply", ["4"]);

        console.log("address", primary.address, "data", data);

        const result = await provider.send("debug_traceCall", [
            {
                to: secondary.address,
                data: data
            }, "latest"
        ]);

        const { calls, output, revertReason } = result;

        // call should be successful
        expect(revertReason).to.equal(null);
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
            }
        ]);

        const { calls, output, revertReason } = result;
        // call should be successful
        expect(revertReason).to.equal(null);

        let outputNumber = primary.interface.decodeFunctionResult("calculate", output);
        console.log("outputNumber", outputNumber);
    });
});