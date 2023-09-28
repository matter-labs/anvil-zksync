import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import { expect } from "chai";
import { Contract, Provider } from "zksync-web3";
import * as hre from "hardhat";
import { HttpNetworkUserConfig } from "hardhat/types";

/**
 * * Deploy a contract using HardHat Deployer
 *
 * @param {Deployer} deployer - HardHat Deployer
 * @param {string} contractName - Name of the contract, without file extension e.g. "Greeter"
 * @param {string[]?} args - Optional arguments to pass to the contract constructor
 *
 *
 * @returns {Promise<Contract>} Returns a promise that resolves to the deployed contract
 * @example
 *      const greeter = await deployContract(deployer, 'Greeter', ['Hi']);
 */
export async function deployContract(deployer: Deployer, contractName: string, args: string[] = []): Promise<Contract> {
  const artifact = await deployer.loadArtifact(contractName);
  return await deployer.deploy(artifact, args);
}

export function getTestProvider(): Provider {
  const network = hre.userConfig.networks?.zkSyncTestnet;
  return new Provider((network as HttpNetworkUserConfig).url);
}

/**
 * * Helper for validating error thrown contains string
 *
 * @param {any} method - Async function to be executed
 * @param {string} errorMessage - String to be searched for in error message
 *
 *
 * @returns {Promise<void>} Returns an empty promise
 * @example
 *      const action = async () => {
 *          await doSomething();
 *          throw new Error("Error: Failure 100");
 *      };
 *      await expectThrowsAsync(action, "Failure 100");
 */
export async function expectThrowsAsync(
  // eslint-disable-next-line @typescript-eslint/ban-types
  method: Function,
  errorMessage: string
): Promise<void> {
  let error = null;
  try {
    await method();
  } catch (err) {
    error = err;
  }

  expect(error).to.be.an("Error");
  if (errorMessage) {
    expect((error as Error).message).to.include(errorMessage);
  }
}
