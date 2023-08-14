import { expect } from 'chai';
import { Wallet, Provider, Contract } from 'zksync-web3';
import * as hre from 'hardhat';
import { Deployer } from '@matterlabs/hardhat-zksync-deploy';

const RICH_WALLET_PK =
  '0x7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110';

async function deployGreeter(deployer: Deployer): Promise<Contract> {
  const artifact = await deployer.loadArtifact('Greeter');
  return await deployer.deploy(artifact, ['Hi']);
}

describe('Greeter', function () {
  it("Should return the new greeting once it's changed", async function () {
    // const provider = Provider.getDefaultProvider();
    console.log("Latest version");
    console.log("Connecting directly to: http://localhost:8011");
    const provider = new Provider('http://127.0.0.1:8011');
    console.log("Provider created");
    
    const wallet = new Wallet(RICH_WALLET_PK, provider);
    console.log("Wallet connected");
    const deployer = new Deployer(hre, wallet);
    console.log("Deployer configured");
    console.log(provider.connection);

    // const test = await provider.getNetwork();
    // console.log(test);

    console.log("Starting manual curl request");
    const http = require('http');

    const data = JSON.stringify({
      jsonrpc: "2.0",
      id: "2",
      method: "eth_call",
      params: [{
        to: "0x36615Cf349d7F6344891B1e7CA7C72883F5dc049",
        data: "0x0000",
        from: "0xa61464658AfeAf65CccaaFD3a512b69A83B77618",
        gas: "0x0000",
        gasPrice: "0x0000",
        value: "0x0000",
        nonce: "0x0000"
      }, "latest"]
    });

    const options = {
      hostname: '127.0.0.1',
      port: 8011,
      path: '/',
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Content-Length': data.length
      }
    };

    const req = http.request(options, (res) => {
      let chunks = [];

      res.on('data', (chunk) => {
        // @ts-ignore
        chunks.push(chunk);
      });

      res.on('end', () => {
        const responseBody = Buffer.concat(chunks).toString('utf-8');
        console.log(responseBody);
      });
    });

    req.on('error', (error) => {
      console.error('Error:', error.message);
    });

    req.write(data);
    req.end();











    

    const greeter = await deployGreeter(deployer);
    console.log("Greeting deployed");

    expect(await greeter.greet()).to.eq('Hi');

    const setGreetingTx = await greeter.setGreeting('Hola, mundo!');
    // wait until the transaction is mined
    await setGreetingTx.wait();

    expect(await greeter.greet()).to.equal('Hola, mundo!');
  });
});
