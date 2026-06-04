# Deep Interview Transcript Summary: TFY Problem Discussion

Metadata:
- Profile: standard
- Context type: greenfield
- Final ambiguity: ~12.5%
- Context snapshot: `.omx/context/tfy-problem-discussion-20260604T005155Z.md`

## Rounds

1. **Problem priority**
   - User wants to discuss token reduction methods and resulting AI performance degradation.

2. **Compression layer**
   - User focused on selective context / structural summary and asked whether they are the same.
   - Clarified distinction: selective context decides what to show; structural summary decides how omitted or included code is represented.

3. **Primary failure mode**
   - User selected `missing-needed-code`: AI may not receive code required for correct work.

4. **Safe expansion policy**
   - User prefers names-first disclosure: initially provide function/scope names, let AI judge what to inspect, and if ambiguity remains, fetch the whole relevant/full context.
