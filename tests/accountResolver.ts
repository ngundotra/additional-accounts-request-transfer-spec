import {
  PublicKey,
  Connection,
  AccountMeta,
  AccountInfo,
} from "@solana/web3.js";
import { getAssociatedTokenAddressSync } from "@solana/spl-token";

let imaginaryIDLForCPIs: CpiIDL[] = [
  {
    __type: "switch",
    condition: {
      __type: "or",
      conditions: [
        {
          __type: "eq",
          keys: [
            {
              __type: "accountInfo",
              account: "mint",
              field: "owner",
            },
            "BPFLoader2111111111111111111111111111111111",
          ],
        },
        {
          __type: "eq",
          keys: [
            {
              __type: "accountInfo",
              account: "mint",
              field: "owner",
            },
            "BPFLoaderUpgradeab1e11111111111111111111111",
          ],
        },
      ],
    },
    false: [
      {
        __type: "invoke",
        program: { __type: "accountInfo", account: "mint", field: "owner" },
        accounts: [
          { __type: "ata", mint: "mint", owner: "owner" },
          "mint",
          { __type: "ata", mint: "mint", owner: "to" },
          "authority",
        ],
        discriminant: {
          __type: "literal",
          value: [12],
        },
      },
    ],
    true: [
      {
        __type: "idl-invoke",
        program: "mint",
        accounts: {
          to: "to",
          owner: "owner",
          authority: "authority",
          mint: "mint",
        },
        method: "transfer",
        arguments: { amount: "amount" },
      },
    ],
  },
];

type CpiIDL = ConditionalCPI | IdlInvoke | RawInvoke;

type ConditionalCPI = {
  __type: "switch";
  condition: ConditionalCPICondition;
  true: CpiIDL[];
  false: CpiIDL[];
};

type CPIAccount = CPIAccountInfo | CPIAccountAta | string;
type CPIAccountAta = {
  __type: "ata";
  mint: string;
  owner: string;
};
type CPIAccountInfo = {
  __type: "accountInfo";
  account: string;
  field: string;
};

type ConditionalCPICondition = ConditionalCPIAndOr | ConditionalCPIEquals;
type ConditionalCPIAndOr = {
  __type: "or" | "and";
  conditions: ConditionalCPICondition[];
};
type ConditionalCPIEquals = {
  __type: "eq";
  keys: ConditionalCPIEqualsAccountInfoCheck[];
};
type ConditionalCPIEqualsAccountInfoCheck = CPIAccountInfo | string;

type IdlInvoke = {
  __type: "idl-invoke";
  program: string;
  method: string;
  arguments: Record<string, string>;
  accounts: Record<string, CPIAccount>;
};

type RawInvoke = {
  __type: "invoke";
  program: CPIAccount;
  discriminant: InvokeDiscriminant;
  accounts: CPIAccount[];
};

type InvokeDiscriminant = {
  __type: "literal";
  value: number[];
};

let cachedAccountInfos = new Map<string, AccountInfo<Buffer>>();
function makeGetAccountInfo(
  connection: Connection
): (pubkey: PublicKey) => Promise<AccountInfo<Buffer>> {
  return async (pubkey: PublicKey) => {
    if (cachedAccountInfos.has(pubkey.toBase58())) {
      return cachedAccountInfos.get(pubkey.toBase58())!;
    } else {
      let accountInfo = await connection.getAccountInfo(pubkey);
      cachedAccountInfos.set(pubkey.toBase58(), accountInfo);
      return accountInfo;
    }
  };
}

async function resolveRemainingAccounts(
  context: Record<string, PublicKey>,
  idl: CpiIDL[],
  getAccountInfo: (pubkey: PublicKey) => Promise<AccountInfo<Buffer>>
): Promise<[AccountMeta[], string[]]> {
  // let accountMap = new Map<string, AccountMeta>();
  let remainingAccountsOrder: string[] = [];
  let remainingAccounts = new Map<string, AccountMeta>();

  function upgradeWritable(
    accounts: Map<string, AccountMeta>,
    accountOrder: string[],
    meta: AccountMeta,
    addToOrder: boolean = true
  ) {
    if (accounts.has(meta.pubkey.toBase58())) {
      let existing = accounts.get(meta.pubkey.toBase58())!;
      console.log("Existing accountMeta:", meta.pubkey.toBase58(), existing);
      if (!existing.isWritable) {
        existing.isWritable = meta.isWritable;
      }
      accounts.set(meta.pubkey.toBase58(), existing);
    } else {
      if (addToOrder) {
        accountOrder.push(meta.pubkey.toBase58());
      }
      accounts.set(meta.pubkey.toBase58(), meta);
    }
  }

  function isKnownKey(key: PublicKey): boolean {
    return (
      Object.values(context).find((_key) => key.equals(_key)) !== undefined
    );
  }

  for (const cpiIdl of idl) {
    console.log("Evaulating:", cpiIdl);
    switch (cpiIdl.__type) {
      case "switch":
        let conditionResult = await evaluateCondition(
          context,
          cpiIdl.condition,
          getAccountInfo
        );
        let conditionSide: string;
        if (conditionResult) {
          conditionSide = "true";
        } else {
          conditionSide = "false";
        }
        let [_orderedMetas, _order] = await resolveRemainingAccounts(
          context,
          cpiIdl[conditionSide],
          getAccountInfo
        );

        // Collapse new info
        _order.forEach((key, index) => {
          remainingAccountsOrder.push(key);
          upgradeWritable(
            remainingAccounts,
            remainingAccountsOrder,
            _orderedMetas[index],
            false
          );
        });
        break;
      case "idl-invoke":
        break;
      case "invoke":
        let program = await resolveAccount(
          context,
          cpiIdl.program,
          getAccountInfo,
          false
        );
        if (!isKnownKey(program.pubkey)) {
          upgradeWritable(remainingAccounts, remainingAccountsOrder, program);
        }

        for (const account of cpiIdl.accounts) {
          const resolved = await resolveAccount(
            context,
            account,
            getAccountInfo
          );
          if (!isKnownKey(resolved.pubkey)) {
            upgradeWritable(
              remainingAccounts,
              remainingAccountsOrder,
              resolved
            );
          }
        }
        break;
    }
  }
  let orderedMetas = remainingAccountsOrder.map(
    (key) => remainingAccounts.get(key)!
  );
  console.log("Order:", remainingAccountsOrder);
  return [orderedMetas, remainingAccountsOrder];
}

async function evaluateCondition(
  context: Record<string, PublicKey>,
  condition: ConditionalCPICondition,
  getAccountInfo: (pubkey: PublicKey) => Promise<AccountInfo<Buffer>>
): Promise<boolean> {
  switch (condition.__type) {
    case "or":
      for (const cond of condition.conditions) {
        if (await evaluateCondition(context, cond, getAccountInfo)) {
          return true;
        }
      }
      return false;
    case "and":
      for (const cond of condition.conditions) {
        if (!(await evaluateCondition(context, cond, getAccountInfo))) {
          return false;
        }
      }
      return true;
    case "eq":
      if (condition.keys.length === 1) {
        return true;
      } else {
        let baseKey = await resolveKey(
          context,
          condition.keys[0],
          getAccountInfo
        );
        for (const condKey of condition.keys.slice(1)) {
          let key = await resolveKey(context, condKey, getAccountInfo);
          // console.log("Found key", key, condKey, baseKey);
          if (!key.equals(baseKey)) {
            return false;
          }
        }
        return true;
      }
  }
}
async function resolveKey(
  context: Record<string, PublicKey>,
  key: ConditionalCPIEqualsAccountInfoCheck,
  getAccountInfo: (pubkey: PublicKey) => Promise<AccountInfo<Buffer>>
): Promise<PublicKey> {
  if (typeof key === "string") {
    return new PublicKey(key);
  } else {
    let accountInfo = await getAccountInfo(context[key.account]);
    return accountInfo[key.field];
  }
}

async function resolveAccount(
  context: Record<string, PublicKey>,
  account: CPIAccount,
  getAccountInfo: (pubkey: PublicKey) => Promise<AccountInfo<Buffer>>,
  isWritable: boolean = true
): Promise<AccountMeta> {
  if (typeof account === "string") {
    return { pubkey: context[account], isSigner: false, isWritable: true };
  } else {
    switch (account.__type) {
      case "ata":
        return {
          pubkey: getAssociatedTokenAddressSync(
            context[account.mint],
            context[account.owner]
          ),
          isSigner: false,
          isWritable,
        };
      case "accountInfo":
        let accountInfo = await getAccountInfo(context[account.account]);
        return {
          pubkey: accountInfo[account.field],
          isSigner: false,
          isWritable,
        };
    }
  }
}
