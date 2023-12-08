import { expect } from "chai";
import { getTestProvider } from "../helpers/utils";

const provider = getTestProvider();

describe("web3_clientVersion", function () {
  it("Should return zkSync/v2.0", async function () {
    // Arrange
    const expectedClientVersion = "zkSync/v2.0";

    // Act
    const response: string = await provider.send("web3_clientVersion", []);

    // Assert
    expect(response).to.equal(expectedClientVersion);
  });
});
