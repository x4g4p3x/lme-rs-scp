"""Compatibility with unmodified official SCP client source at a pinned commit."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile

UPSTREAM_REVISION = "cea5398564032aea65a78e246d06c30ae945e03f"
parser = argparse.ArgumentParser()
parser.add_argument("--source", type=Path, required=True)
parser.add_argument("--binary", type=Path, required=True)
args = parser.parse_args()
revision = subprocess.check_output(["git", "-C", str(args.source), "rev-parse", "HEAD"], text=True).strip()
assert revision == UPSTREAM_REVISION, revision
sys.path.insert(0, str(args.source / "src"))

import anyio
from pydantic import AnyUrl
from scp import ClientSession, StdioServerParameters, stdio_client
from scp.shared.exceptions import ScpError

ROOT = Path(__file__).resolve().parent

def document(result):
    assert not result.isError, result
    text = json.loads(next(block.text for block in result.content if block.type == "text"))
    extra = result.model_dump(by_alias=True).get("structuredContent")
    if extra is not None:
        assert extra == text
    return text

async def main():
    with anyio.fail_after(60):
        params = StdioServerParameters(command=str(args.binary.resolve()), env={"LME_DATA_ROOT":str(ROOT / "data")})
        async with stdio_client(params) as (read, write):
            async with ClientSession(read, write) as session:
                info = await session.initialize()
                assert info.capabilities.resources is not None
                tools = await session.list_tools()
                assert {t.name for t in tools.tools} == {"fit_model","assess_model","infer_marginals","export_experiment","forget_resource"}
                fitted = document(await session.call_tool("fit_model", {"model_kind":"lmm","formula":"y ~ A * B + x + (1 | subject)","data_path":"marginal.csv","reml":False,"context":{"experiment_id":"official-client","run_id":"run:1","objective":"Assess treatment"}}))
                model_id = fitted["resource"]["summary"]["fit_id"]
                assert fitted["provenance"]["engine"]["version"] == "0.2.7"
                assert fitted["provenance"]["agent"]["version"] == "0.2.1"
                listed = await session.list_resources()
                assert fitted["resource"]["uri"] in {str(r.uri) for r in listed.resources}
                loaded = await session.read_resource(AnyUrl(fitted["resource"]["uri"]))
                assert json.loads(loaded.contents[0].text) == fitted
                assessed = document(await session.call_tool("assess_model", {"fit_id":model_id,"cross_validation":{"group_col":"subject","n_splits":2,"seed":42}}))
                inferred = document(await session.call_tool("infer_marginals", {"fit_id":model_id,"terms":["A"],"by":["B"],"adjust":"holm"}))
                assert assessed["context"] == inferred["context"] == fitted["context"]
                assert len(inferred["resource"]["summary"]["means"]["rows"]) == 6
                bundle = document(await session.call_tool("export_experiment", {"experiment_id":"official-client","include_data":True}))
                assert len(bundle["resources"]) == 3
                assert len(bundle["datasets"]) == 1
                assert bundle["datasets"][0]["csv"].encode() == (ROOT / "data/marginal.csv").read_bytes()
                with tempfile.TemporaryDirectory(prefix="lme-scp-outside-") as outside:
                    outside_path = Path(outside) / "input.csv"
                    outside_path.write_text("y,x\n1,0\n2,1\n3,2\n")
                    rejected = await session.call_tool("fit_model",{"model_kind":"lm","formula":"y ~ x","data_path":str(outside_path)})
                    assert rejected.isError
                    assert "outside LME_DATA_ROOT" in rejected.content[0].text
                for name, arguments in [("fit_model",{"model_kind":"unknown"}),("infer_marginals",{"fit_id":model_id,"terms":["missing"]})]:
                    try:
                        result = await session.call_tool(name,arguments)
                        assert result.isError
                    except ScpError:
                        pass
                assert len((await session.list_tools()).tools) == 5
                forgotten = document(await session.call_tool("forget_resource",{"uri":fitted["resource"]["uri"]}))
                assert len(forgotten["uris"]) == 3
                assert not (await session.list_resources()).resources
        print(f"Official SCP client {UPSTREAM_REVISION}: workflow, resources, provenance, export, errors, and cleanup passed")

anyio.run(main)
