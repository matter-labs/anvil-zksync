## Overview: `release.yaml`

The release pipeline is aimed at automating the creation of GitHub releases, ensuring consistency and efficiency in our delivery process. The workflow is inspired by practices from [reth](https://github.com/paradigmxyz/reth/blob/main/.github/workflows/release.yml) and  [Lighthouse](https://github.com/sigp/lighthouse/blob/693886b94176faa4cb450f024696cb69cda2fe58/.github/workflows/release.yml).

### Release Workflow

1. **Extract version**: The workflow extracts the version from the Git tag. The Git tag is being used to trigger the workflow.
2. **Build**: A matrix build is executed to generate binaries for multiple architectures.
3. **Draft Release**: A release draft is prepared with an automatically generated changelog and attached binaries.

### First Run

On the first execution of this workflow, the "All Changes" section of the release draft will be empty. This behaviour occurs because the action fetches commits between tags. Since there will be no prior tag for the first run, it does not populate this section. However, for subsequent runs, this section will feature all commits up to the given tag. The first run we will have to do so manually by running `git log main --oneline` and adding the commits. 

### Draft Release

The workflow creates a **draft release** instead of a public one. This approach is intentional. It allows us to:

- Add any additional commentary.
- Ensure checks and test CI jobs pass successfully.
- Prepare announcements or tweets related to the release prior to the release. 
- Ensure it works as intended, and all the changes intended are present.
  
Once the draft is reviewed and any required changes are made, we can finalize and create the release accordingly. 

**Note:** The binaries links in the table will not resolve until the release is made. That is, when a release is in "draft" form on GitHub, it is given an untagged-... URL. So when the release is made it will be tagged with the corresponding tag. 

### Triggering the Release Pipeline

To trigger the release pipeline, you'll need to create and push a Git tag. Here are the commands:

```bash
make new-release-tag
# Be sure to run the 'git push' command included in the output
```

The version number comes from [Cargo.toml](../Cargo.toml). This will trigger the `check.yaml` and `test.yaml` workflows, and `release.yaml`.