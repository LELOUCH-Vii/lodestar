#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, vec, Address, Env, String, Vec,
};

const MAX_TTL: u32 = 3110400;

#[contracttype]
#[derive(Clone)]
pub struct ServiceEntry {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub endpoint: String,
    pub price_usdc: String,
    pub category: String,
    pub provider: Address,
    pub reputation: i32,
    pub active: bool,
    pub registered_at: u64,
}

#[contracttype]
pub enum DataKey {
    Counter,
    ServiceIds,
    Service(u64),
    ServiceIdsByCategory(String),
}

#[contract]
pub struct LodestarRegistry;

#[contractimpl]
impl LodestarRegistry {
    pub fn register_service(
        env: Env,
        provider: Address,
        name: String,
        description: String,
        endpoint: String,
        price_usdc: String,
        category: String,
    ) -> u64 {
        provider.require_auth();

        let counter: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Counter)
            .unwrap_or(0u64);

        let new_id = counter + 1;

        let cat = category.clone();

        let entry = ServiceEntry {
            id: new_id,
            name,
            description,
            endpoint,
            price_usdc,
            category,
            provider,
            reputation: 0,
            active: true,
            registered_at: env.ledger().sequence() as u64,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Service(new_id), &entry);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Service(new_id), MAX_TTL, MAX_TTL);

        env.storage()
            .persistent()
            .set(&DataKey::Counter, &new_id);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Counter, MAX_TTL, MAX_TTL);

        let mut ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::ServiceIds)
            .unwrap_or_else(|| vec![&env]);
        ids.push_back(new_id);
        env.storage()
            .persistent()
            .set(&DataKey::ServiceIds, &ids);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::ServiceIds, MAX_TTL, MAX_TTL);

        let mut cat_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::ServiceIdsByCategory(cat.clone()))
            .unwrap_or_else(|| vec![&env]);
        cat_ids.push_back(new_id);
        env.storage()
            .persistent()
            .set(&DataKey::ServiceIdsByCategory(cat.clone()), &cat_ids);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::ServiceIdsByCategory(cat), MAX_TTL, MAX_TTL);

        new_id
    }

    pub fn get_service(env: Env, id: u64) -> ServiceEntry {
        env.storage()
            .persistent()
            .get(&DataKey::Service(id))
            .expect("Service not found")
    }

    pub fn list_services_page(
        env: Env,
        page: u32,
        page_size: u32,
        category: Option<String>,
    ) -> Vec<ServiceEntry> {
        let page_size = page_size.min(20u32).max(1u32);
        let start: u32 = page * page_size;

        let ids: Vec<u64> = if let Some(ref cat) = category {
            env.storage()
                .persistent()
                .get(&DataKey::ServiceIdsByCategory(cat.clone()))
                .unwrap_or_else(|| vec![&env])
        } else {
            env.storage()
                .persistent()
                .get(&DataKey::ServiceIds)
                .unwrap_or_else(|| vec![&env])
        };

        let total = ids.len();
        let end = (start + page_size).min(total);

        let mut services: Vec<ServiceEntry> = vec![&env];
        let mut i = start;
        while i < end {
            if let Some(entry) = env
                .storage()
                .persistent()
                .get::<DataKey, ServiceEntry>(&DataKey::Service(ids.get(i).unwrap()))
            {
                if entry.active {
                    services.push_back(entry);
                }
            }
            i += 1;
        }

        // Insertion sort by reputation descending
        let len = services.len();
        for i in 1..len {
            let mut j = i;
            while j > 0 {
                let a = services.get(j - 1).unwrap();
                let b = services.get(j).unwrap();
                if a.reputation >= b.reputation {
                    break;
                }
                services.set(j - 1, b);
                services.set(j, a);
                j -= 1;
            }
        }

        services
    }

    pub fn update_reputation(env: Env, id: u64, positive: bool) {
        let mut entry: ServiceEntry = env
            .storage()
            .persistent()
            .get(&DataKey::Service(id))
            .expect("Service not found");

        if positive {
            entry.reputation += 1;
        } else {
            entry.reputation -= 1;
        }

        env.storage()
            .persistent()
            .set(&DataKey::Service(id), &entry);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Service(id), MAX_TTL, MAX_TTL);
    }

    pub fn deactivate_service(env: Env, provider: Address, id: u64) {
        provider.require_auth();

        let mut entry: ServiceEntry = env
            .storage()
            .persistent()
            .get(&DataKey::Service(id))
            .expect("Service not found");

        assert!(
            provider == entry.provider,
            "Only the provider can deactivate this service"
        );

        entry.active = false;
        env.storage()
            .persistent()
            .set(&DataKey::Service(id), &entry);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Service(id), MAX_TTL, MAX_TTL);

        // Remove from category index
        let cat_key = DataKey::ServiceIdsByCategory(entry.category.clone());
        let cat_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&cat_key)
            .expect("Category index not found");
        let mut updated: Vec<u64> = vec![&env];
        for cid in cat_ids.iter() {
            if cid != id {
                updated.push_back(cid);
            }
        }
        env.storage().persistent().set(&cat_key, &updated);
        env.storage()
            .persistent()
            .extend_ttl(&cat_key, MAX_TTL, MAX_TTL);
    }

    pub fn get_service_count(env: Env) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::Counter)
            .unwrap_or(0u64)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, String};

    fn setup_service(env: &Env, id: u64, provider: &Address, category: &str, reputation: i32, active: bool) {
        let cat = String::from_str(env, category);
        let entry = ServiceEntry {
            id,
            name: String::from_str(env, "Test Service"),
            description: String::from_str(env, "Test Description"),
            endpoint: String::from_str(env, "https://test.com"),
            price_usdc: String::from_str(env, "10"),
            category: cat.clone(),
            provider: provider.clone(),
            reputation,
            active,
            registered_at: env.ledger().sequence() as u64,
        };
        env.storage().persistent().set(&DataKey::Service(id), &entry);
        
        // Add to ServiceIds list
        let mut ids: Vec<u64> = env.storage().persistent().get(&DataKey::ServiceIds).unwrap_or_else(|| vec![env]);
        ids.push_back(id);
        env.storage().persistent().set(&DataKey::ServiceIds, &ids);

        // Add to category index
        let mut cat_ids: Vec<u64> = env.storage().persistent().get(&DataKey::ServiceIdsByCategory(cat.clone())).unwrap_or_else(|| vec![env]);
        cat_ids.push_back(id);
        env.storage().persistent().set(&DataKey::ServiceIdsByCategory(cat), &cat_ids);
    }

    #[test]
    fn test_list_services_empty() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LodestarRegistry);
        
        env.clone().as_contract(&contract_id, || {
            // Test with no services registered
            let result = LodestarRegistry::list_services_page(env.clone(), 0, 20, None);
            assert_eq!(result.len(), 0);
        });
    }

    #[test]
    fn test_list_services_single_entry() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LodestarRegistry);
        
        env.clone().as_contract(&contract_id, || {
            let provider = Address::generate(&env);
            setup_service(&env, 1, &provider, "compute", 0, true);

            // Test listing all services
            let result = LodestarRegistry::list_services_page(env, 0, 20, None);
            assert_eq!(result.len(), 1);
            assert_eq!(result.get(0).unwrap().id, 1);
        });
    }

    #[test]
    fn test_list_services_reputation_sorting() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LodestarRegistry);
        
        env.clone().as_contract(&contract_id, || {
            let provider = Address::generate(&env);

            // Register three services with different reputations
            setup_service(&env, 1, &provider, "compute", 2, true);
            setup_service(&env, 2, &provider, "compute", 1, true);
            setup_service(&env, 3, &provider, "compute", -1, true);

            // Test sorting (should be descending: 1=2, 2=1, 3=-1)
            let result = LodestarRegistry::list_services_page(env, 0, 20, None);
            assert_eq!(result.len(), 3);
            assert_eq!(result.get(0).unwrap().id, 1);
            assert_eq!(result.get(1).unwrap().id, 2);
            assert_eq!(result.get(2).unwrap().id, 3);
        });
    }

    #[test]
    fn test_list_services_tied_reputation() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LodestarRegistry);
        
        env.clone().as_contract(&contract_id, || {
            let provider = Address::generate(&env);

            // Register three services with same reputation
            setup_service(&env, 1, &provider, "compute", 1, true);
            setup_service(&env, 2, &provider, "compute", 1, true);
            setup_service(&env, 3, &provider, "compute", 1, true);

            // Test that all are returned (order may vary for ties)
            let result = LodestarRegistry::list_services_page(env, 0, 20, None);
            assert_eq!(result.len(), 3);
            
            // Verify all have same reputation
            let rep1 = result.get(0).unwrap().reputation;
            let rep2 = result.get(1).unwrap().reputation;
            let rep3 = result.get(2).unwrap().reputation;
            assert_eq!(rep1, rep2);
            assert_eq!(rep2, rep3);
        });
    }

    #[test]
    fn test_list_services_category_filter() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LodestarRegistry);
        
        env.clone().as_contract(&contract_id, || {
            let provider = Address::generate(&env);

            // Register services in different categories
            setup_service(&env, 1, &provider, "compute", 0, true);
            setup_service(&env, 2, &provider, "storage", 0, true);
            setup_service(&env, 3, &provider, "compute", 0, true);

            // Test filtering by compute category
            let compute_result = LodestarRegistry::list_services_page(
                env.clone(),
                0,
                20,
                Some(String::from_str(&env, "compute")),
            );
            assert_eq!(compute_result.len(), 2);

            // Test filtering by storage category
            let storage_result = LodestarRegistry::list_services_page(
                env.clone(),
                0,
                20,
                Some(String::from_str(&env, "storage")),
            );
            assert_eq!(storage_result.len(), 1);
            assert_eq!(storage_result.get(0).unwrap().id, 2);

            // Test with no filter (should return all)
            let all_result = LodestarRegistry::list_services_page(env, 0, 20, None);
            assert_eq!(all_result.len(), 3);
        });
    }

    #[test]
    fn test_list_services_inactive_filtered() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LodestarRegistry);
        
        env.clone().as_contract(&contract_id, || {
            let provider = Address::generate(&env);

            // Register two services, one active and one inactive
            setup_service(&env, 1, &provider, "compute", 0, true);
            setup_service(&env, 2, &provider, "compute", 0, false);

            // Test that only active service is returned
            let result = LodestarRegistry::list_services_page(env, 0, 20, None);
            assert_eq!(result.len(), 1);
            assert_eq!(result.get(0).unwrap().id, 1);
        });
    }

    #[test]
    fn test_list_services_category_filter_with_reputation() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LodestarRegistry);
        
        env.clone().as_contract(&contract_id, || {
            let provider = Address::generate(&env);

            // Register services in different categories with different reputations
            setup_service(&env, 1, &provider, "compute", 1, true);
            setup_service(&env, 2, &provider, "compute", 2, true);
            setup_service(&env, 3, &provider, "storage", 1, true);

            // Test filtering by compute category with reputation sorting
            let compute_result = LodestarRegistry::list_services_page(
                env.clone(),
                0,
                20,
                Some(String::from_str(&env, "compute")),
            );
            assert_eq!(compute_result.len(), 2);
            assert_eq!(compute_result.get(0).unwrap().id, 2); // Higher reputation
            assert_eq!(compute_result.get(1).unwrap().id, 1);
        });
    }

    #[test]
    fn test_list_services_nonexistent_category() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LodestarRegistry);
        
        env.clone().as_contract(&contract_id, || {
            let provider = Address::generate(&env);

            // Register a service
            setup_service(&env, 1, &provider, "compute", 0, true);

            // Test filtering by non-existent category
            let result = LodestarRegistry::list_services_page(
                env.clone(),
                0,
                20,
                Some(String::from_str(&env, "nonexistent")),
            );
            assert_eq!(result.len(), 0);
        });
    }
}
