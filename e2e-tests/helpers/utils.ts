import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import { Contract } from "zksync-web3";

/**
 * * Deploy a contract using HardHat Deployer
 *
 * @param {Deployer} deployer  - HardHat Deployer
 * @param {string} contractName - Name of the contract, without file extension e.g. "Greeter"
 * @param {string[]?} args - Optional arguments to pass to the contract constructor
 *
 *
 * @returns {Promise<Contract>} Returns a promise that resolves to the deployed contract
 * @example
 *      const greeter = await deployContract(deployer, 'Greeter', ['Hi']);
 */
export async function deployContract(
  deployer: Deployer,
  contractName: string,
  args: string[] = [],
): Promise<Contract> {
  const artifact = await deployer.loadArtifact(contractName);
  return await deployer.deploy(artifact, args);
}
